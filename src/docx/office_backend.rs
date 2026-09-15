use super::backend::DocxBackend;
use super::types::{DocxError, DocxMetadata, check_dpi, check_input};
use crate::common::ImageBuffer;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Read};
use std::sync::Mutex;

pub struct OfficeDocxBackend;

/// Last converted PDF, keyed by content hash. The viewer renders one page
/// per frame from the same bytes, so without this every page pays a full
/// office2pdf conversion (~200ms on the 180-page fixture); with it only
/// the first does. Single entry keeps memory bounded — one document at a
/// time is the viewer norm.
static PDF_CACHE: Mutex<Option<(u64, Vec<u8>)>> = Mutex::new(None);

fn cache_key(docx: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    docx.hash(&mut hasher);
    hasher.finish()
}

fn cached_pdf(docx: &[u8]) -> Option<Vec<u8>> {
    let key = cache_key(docx);
    PDF_CACHE
        .lock()
        .ok()?
        .as_ref()
        .filter(|(k, _)| *k == key)
        .map(|(_, pdf)| pdf.clone())
}

fn store_pdf(docx: &[u8], pdf: Vec<u8>) -> Vec<u8> {
    if let Ok(mut cache) = PDF_CACHE.lock() {
        *cache = Some((cache_key(docx), pdf.clone()));
    }
    pdf
}

impl OfficeDocxBackend {
    fn archive(docx: &[u8]) -> Result<zip::ZipArchive<Cursor<&[u8]>>, DocxError> {
        zip::ZipArchive::new(Cursor::new(docx))
            .map_err(|e| DocxError::Parse(format!("not a zip/docx: {e}")))
    }

    fn read_entry(docx: &[u8], path: &str) -> Result<String, DocxError> {
        let mut archive = Self::archive(docx)?;
        let mut file = archive
            .by_name(path)
            .map_err(|e| DocxError::Parse(format!("missing {path}: {e}")))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| DocxError::Parse(format!("cannot read {path}: {e}")))?;
        Ok(content)
    }

    fn decode_text(text: &quick_xml::events::BytesText<'_>) -> Option<String> {
        let decoded = text.decode().ok()?;
        let unescaped = quick_xml::escape::unescape(decoded.as_ref()).ok()?;
        Some(unescaped.into_owned())
    }

    /// Single pass over `word/document.xml`: paragraph/table counts plus the
    /// paragraph texts (concatenated `<w:t>` runs each). Headers, footers
    /// and footnotes live in other parts and are intentionally out of scope —
    /// `probe` reports body stats, matching what the viewer renders.
    ///
    /// Exposed for the node tests: lets them assert exact XML-level counts
    /// on hand-rolled fixtures without paying a conversion.
    pub fn analyze(xml: &str) -> (usize, usize, Vec<String>) {
        let mut reader = quick_xml::Reader::from_str(xml);
        let mut paragraphs = 0;
        let mut tables = 0;
        let mut done: Vec<String> = Vec::new();
        let mut current: Option<Vec<String>> = None;
        let mut in_t = false;
        macro_rules! push_run {
            ($run:expr) => {
                if let Some(runs) = current.as_mut() {
                    runs.push($run);
                } else {
                    done.push($run);
                }
            };
        }
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    let name = e.local_name();
                    if name.as_ref() == b"p" {
                        paragraphs += 1;
                        if let Some(runs) = current.take() {
                            let text = runs.concat();
                            if !text.is_empty() {
                                done.push(text);
                            }
                        }
                        current = Some(Vec::new());
                    } else if name.as_ref() == b"tbl" {
                        tables += 1;
                    }
                    in_t = name.as_ref() == b"t";
                }
                Ok(quick_xml::events::Event::Empty(ref e)) => {
                    let name = e.local_name();
                    if name.as_ref() == b"p" {
                        paragraphs += 1;
                    } else if name.as_ref() == b"tbl" {
                        tables += 1;
                    }
                    in_t = false;
                }
                Ok(quick_xml::events::Event::Text(ref t)) => {
                    if in_t
                        && let Some(s) = Self::decode_text(t)
                        && !s.is_empty()
                    {
                        push_run!(s);
                    }
                }
                Ok(quick_xml::events::Event::End(ref e)) => {
                    if e.local_name().as_ref() == b"p"
                        && let Some(runs) = current.take()
                    {
                        let text = runs.concat();
                        if !text.is_empty() {
                            done.push(text);
                        }
                    }
                    in_t = false;
                }
                Ok(quick_xml::events::Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        if let Some(runs) = current.take() {
            let text = runs.concat();
            if !text.is_empty() {
                done.push(text);
            }
        }
        (paragraphs, tables, done)
    }

    fn pdf_pages(pdf: &[u8]) -> Result<usize, DocxError> {
        #[cfg(feature = "pdf-hayro")]
        {
            crate::pdf::page_count(pdf).map_err(|e| DocxError::Render(e.to_string()))
        }
        #[cfg(all(not(feature = "pdf-hayro"), feature = "pdf-zpdf"))]
        {
            crate::pdf::page_count(pdf).map_err(|e| DocxError::Render(e.to_string()))
        }
        #[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
        {
            let _ = pdf;
            Err(DocxError::NoBackend)
        }
    }

    fn rasterize_pdf(pdf: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, DocxError> {
        check_dpi(dpi)?;
        #[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
        {
            crate::pdf::rasterize_page(pdf, page, dpi).map_err(|e| match e {
                crate::pdf::PdfError::PageOutOfRange { requested, total } => {
                    DocxError::PageOutOfRange { requested, total }
                }
                crate::pdf::PdfError::InvalidDpi(dpi) => DocxError::InvalidDpi(dpi),
                other => DocxError::Render(other.to_string()),
            })
        }
        #[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
        {
            let _ = (pdf, page);
            Err(DocxError::NoBackend)
        }
    }
}

impl DocxBackend for OfficeDocxBackend {
    fn name(&self) -> &'static str {
        "office2pdf"
    }

    fn probe(&self, docx: &[u8]) -> Result<DocxMetadata, DocxError> {
        check_input(docx)?;
        let xml = Self::read_entry(docx, "word/document.xml")?;
        let (paragraphs, tables, texts) = Self::analyze(&xml);
        let words = texts.join(" ").split_whitespace().count();
        let pages = self.page_count(docx)?;
        Ok(DocxMetadata {
            // Contract: counts are at least 1 — see `HayroBackend::probe`.
            pages: pages.max(1),
            paragraphs,
            tables,
            words,
        })
    }

    fn page_count(&self, docx: &[u8]) -> Result<usize, DocxError> {
        let pdf = self.to_pdf(docx)?;
        // Contract: page counts are at least 1 — see `HayroBackend::probe`.
        // `pdf_pages` funnels through `crate::pdf`, whose backends clamp.
        Ok(Self::pdf_pages(&pdf)?.max(1))
    }

    fn to_pdf(&self, docx: &[u8]) -> Result<Vec<u8>, DocxError> {
        check_input(docx)?;
        #[cfg(feature = "docx-office2pdf")]
        {
            if let Some(pdf) = cached_pdf(docx) {
                return Ok(pdf);
            }
            let pdf = office2pdf::convert_bytes(
                docx,
                office2pdf::config::Format::Docx,
                &office2pdf::config::ConvertOptions::default(),
            )
            .map(|result| result.pdf)
            .map_err(|e| DocxError::Convert(format!("{e:?}")))?;
            Ok(store_pdf(docx, pdf))
        }
        #[cfg(not(feature = "docx-office2pdf"))]
        {
            Err(DocxError::NoBackend)
        }
    }

    fn rasterize_page(&self, docx: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, DocxError> {
        check_input(docx)?;
        check_dpi(dpi)?;
        let total = self.page_count(docx)?;
        if page >= total {
            return Err(DocxError::PageOutOfRange {
                requested: page,
                total,
            });
        }
        let pdf = self.to_pdf(docx)?;
        Self::rasterize_pdf(&pdf, page, dpi)
    }

    fn rasterize_all(&self, docx: &[u8], dpi: u32) -> Result<Vec<ImageBuffer>, DocxError> {
        check_input(docx)?;
        check_dpi(dpi)?;
        let pdf = self.to_pdf(docx)?;
        #[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
        {
            let total = Self::pdf_pages(&pdf)?;
            let mut out = Vec::with_capacity(total);
            for page in 0..total {
                out.push(Self::rasterize_pdf(&pdf, page, dpi)?);
            }
            Ok(out)
        }
        #[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
        {
            let _ = pdf;
            Err(DocxError::NoBackend)
        }
    }

    fn extract_text(&self, docx: &[u8]) -> Result<String, DocxError> {
        check_input(docx)?;
        let xml = Self::read_entry(docx, "word/document.xml")?;
        let (_, _, texts) = Self::analyze(&xml);
        Ok(texts.join("\n"))
    }
}
