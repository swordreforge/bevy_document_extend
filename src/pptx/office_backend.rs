use super::backend::PptxBackend;
use super::types::{PptxError, PptxMetadata, check_dpi, check_input};
use crate::common::ImageBuffer;
use std::io::{Cursor, Read};

pub struct OfficePptxBackend;

impl OfficePptxBackend {
    fn archive(pptx: &[u8]) -> Result<zip::ZipArchive<Cursor<&[u8]>>, PptxError> {
        zip::ZipArchive::new(Cursor::new(pptx))
            .map_err(|e| PptxError::Parse(format!("not a zip/pptx: {e}")))
    }

    /// Slide part names (`ppt/slides/slideN.xml`) sorted by N.
    fn slide_parts(pptx: &[u8]) -> Result<Vec<String>, PptxError> {
        check_input(pptx)?;
        let archive = Self::archive(pptx)?;
        let mut numbered: Vec<(usize, String)> = archive
            .file_names()
            .filter_map(|name| {
                let num: usize = name
                    .strip_prefix("ppt/slides/slide")?
                    .strip_suffix(".xml")?
                    .parse()
                    .ok()?;
                Some((num, name.to_string()))
            })
            .collect();
        numbered.sort();
        Ok(numbered.into_iter().map(|(_, name)| name).collect())
    }

    fn read_entry(pptx: &[u8], path: &str) -> Result<String, PptxError> {
        let mut archive = Self::archive(pptx)?;
        let mut file = archive
            .by_name(path)
            .map_err(|e| PptxError::Parse(format!("missing {path}: {e}")))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| PptxError::Parse(format!("cannot read {path}: {e}")))?;
        Ok(content)
    }

    fn decode_text(text: &quick_xml::events::BytesText<'_>) -> Option<String> {
        let decoded = text.decode().ok()?;
        let unescaped = quick_xml::escape::unescape(decoded.as_ref()).ok()?;
        Some(unescaped.into_owned())
    }

    /// `dc:title` from `docProps/core.xml`; `None` when absent or unreadable.
    /// Never fails `probe` — title is informational only.
    fn doc_title(pptx: &[u8]) -> Option<String> {
        let xml = Self::read_entry(pptx, "docProps/core.xml").ok()?;
        let mut reader = quick_xml::Reader::from_str(&xml);
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Start(ref e))
                    if e.local_name().as_ref() == b"title" =>
                {
                    loop {
                        match reader.read_event() {
                            Ok(quick_xml::events::Event::Text(ref t)) => {
                                return Self::decode_text(t).filter(|s| !s.trim().is_empty());
                            }
                            Ok(quick_xml::events::Event::End(_))
                            | Ok(quick_xml::events::Event::Eof)
                            | Err(_) => return None,
                            _ => {}
                        }
                    }
                }
                Ok(quick_xml::events::Event::Eof) | Err(_) => return None,
                _ => {}
            }
        }
    }

    /// Plain text of one slide: concatenated `<a:t>` runs, one per line.
    fn slide_text(xml: &str) -> String {
        let mut reader = quick_xml::Reader::from_str(xml);
        let mut runs = Vec::new();
        let mut in_t = false;
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Start(ref e)) => {
                    in_t = e.local_name().as_ref() == b"t";
                }
                Ok(quick_xml::events::Event::Text(ref t)) => {
                    if in_t
                        && let Some(s) = Self::decode_text(t)
                        && !s.is_empty()
                    {
                        runs.push(s);
                    }
                }
                Ok(quick_xml::events::Event::End(_)) | Ok(quick_xml::events::Event::Empty(_)) => {
                    in_t = false;
                }
                Ok(quick_xml::events::Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        runs.join("\n")
    }

    fn pdf_pages(pdf: &[u8]) -> Result<usize, PptxError> {
        #[cfg(feature = "pdf-hayro")]
        {
            crate::pdf::page_count(pdf).map_err(|e| PptxError::Render(e.to_string()))
        }
        #[cfg(all(not(feature = "pdf-hayro"), feature = "pdf-zpdf"))]
        {
            crate::pdf::page_count(pdf).map_err(|e| PptxError::Render(e.to_string()))
        }
        #[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
        {
            let _ = pdf;
            Err(PptxError::NoBackend)
        }
    }

    fn rasterize_pdf(pdf: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, PptxError> {
        check_dpi(dpi)?;
        #[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
        {
            crate::pdf::rasterize_page(pdf, page, dpi).map_err(|e| match e {
                crate::pdf::PdfError::PageOutOfRange { requested, total } => {
                    PptxError::PageOutOfRange { requested, total }
                }
                crate::pdf::PdfError::InvalidDpi(dpi) => PptxError::InvalidDpi(dpi),
                other => PptxError::Render(other.to_string()),
            })
        }
        #[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
        {
            let _ = (pdf, page);
            Err(PptxError::NoBackend)
        }
    }
}

impl PptxBackend for OfficePptxBackend {
    fn name(&self) -> &'static str {
        "office2pdf"
    }

    fn probe(&self, pptx: &[u8]) -> Result<PptxMetadata, PptxError> {
        let slides = Self::slide_parts(pptx)?.len();
        Ok(PptxMetadata {
            // Contract: counts are at least 1 — see `HayroBackend::probe`.
            slides: slides.max(1),
            title: Self::doc_title(pptx),
        })
    }

    fn slide_count(&self, pptx: &[u8]) -> Result<usize, PptxError> {
        Ok(Self::slide_parts(pptx)?.len())
    }

    fn extract_text(&self, pptx: &[u8]) -> Result<String, PptxError> {
        check_input(pptx)?;
        let parts = Self::slide_parts(pptx)?;
        let mut slides = Vec::with_capacity(parts.len());
        for part in &parts {
            let xml = Self::read_entry(pptx, part)?;
            let text = Self::slide_text(&xml);
            if !text.trim().is_empty() {
                slides.push(text);
            }
        }
        Ok(slides.join("\n\n"))
    }

    fn to_pdf(&self, pptx: &[u8]) -> Result<Vec<u8>, PptxError> {
        check_input(pptx)?;
        #[cfg(feature = "pptx-office2pdf")]
        {
            office2pdf::convert_bytes(
                pptx,
                office2pdf::config::Format::Pptx,
                &office2pdf::config::ConvertOptions::default(),
            )
            .map(|result| result.pdf)
            .map_err(|e| PptxError::Convert(format!("{e:?}")))
        }
        #[cfg(not(feature = "pptx-office2pdf"))]
        {
            Err(PptxError::NoBackend)
        }
    }

    fn page_count(&self, pptx: &[u8]) -> Result<usize, PptxError> {
        let pdf = self.to_pdf(pptx)?;
        // Contract: page counts are at least 1 — see `HayroBackend::probe`.
        // `pdf_pages` funnels through `crate::pdf`, whose backends clamp.
        Ok(Self::pdf_pages(&pdf)?.max(1))
    }

    fn rasterize_page(&self, pptx: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, PptxError> {
        check_input(pptx)?;
        check_dpi(dpi)?;
        let total = self.page_count(pptx)?;
        if page >= total {
            return Err(PptxError::PageOutOfRange {
                requested: page,
                total,
            });
        }
        let pdf = self.to_pdf(pptx)?;
        Self::rasterize_pdf(&pdf, page, dpi)
    }

    fn rasterize_all(&self, pptx: &[u8], dpi: u32) -> Result<Vec<ImageBuffer>, PptxError> {
        check_input(pptx)?;
        check_dpi(dpi)?;
        let pdf = self.to_pdf(pptx)?;
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
            Err(PptxError::NoBackend)
        }
    }
}
