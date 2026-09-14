use super::backend::DocxBackend;
use super::types::{DocxError, DocxMetadata, check_dpi, check_input};
use crate::common::ImageBuffer;

pub struct RdocxBackend;

impl RdocxBackend {
    fn open(docx: &[u8]) -> Result<rdocx::Document, DocxError> {
        check_input(docx)?;
        rdocx::Document::from_bytes(docx).map_err(|e| DocxError::Parse(format!("{e:?}")))
    }

    fn layout_pages(doc: &rdocx::Document) -> Result<usize, DocxError> {
        let layout = doc
            .layout_deterministic()
            .map_err(|e| DocxError::Layout(format!("{e:?}")))?;
        Ok(layout.layout.pages.len())
    }

    fn layout_page_count(docx: &[u8]) -> Result<usize, DocxError> {
        Self::layout_pages(&Self::open(docx)?)
    }
}

impl DocxBackend for RdocxBackend {
    fn name(&self) -> &'static str {
        "rdocx"
    }

    fn probe(&self, docx: &[u8]) -> Result<DocxMetadata, DocxError> {
        let doc = Self::open(docx)?;
        Ok(DocxMetadata {
            pages: Self::layout_pages(&doc)?,
            paragraphs: doc.paragraph_count(),
            tables: doc.table_count(),
            words: doc.word_count(),
        })
    }

    fn page_count(&self, docx: &[u8]) -> Result<usize, DocxError> {
        Self::layout_page_count(docx)
    }

    fn rasterize_page(&self, docx: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, DocxError> {
        check_input(docx)?;
        let dpi_f = check_dpi(dpi)?;
        let doc = Self::open(docx)?;
        let total = Self::layout_pages(&doc)?;
        let png = doc
            .render_page_to_png_deterministic(page, dpi_f)
            .map_err(|e| DocxError::Render(format!("{e:?}")))?
            .ok_or(DocxError::PageOutOfRange {
                requested: page,
                total,
            })?;
        let dynamic = image::load_from_memory(&png).map_err(|e| DocxError::Image(e.to_string()))?;
        let rgba = dynamic.to_rgba8();
        Ok(ImageBuffer {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        })
    }

    fn extract_text(&self, docx: &[u8]) -> Result<String, DocxError> {
        Ok(Self::open(docx)?.text())
    }

    fn rasterize_all(&self, docx: &[u8], dpi: u32) -> Result<Vec<ImageBuffer>, DocxError> {
        check_input(docx)?;
        let dpi_f = check_dpi(dpi)?;
        let doc = Self::open(docx)?;
        let total = Self::layout_pages(&doc)?;
        let mut out = Vec::with_capacity(total);
        let pngs = doc
            .render_all_pages(dpi_f)
            .map_err(|e| DocxError::Render(format!("{e:?}")))?;
        for png in pngs {
            let dynamic =
                image::load_from_memory(&png).map_err(|e| DocxError::Image(e.to_string()))?;
            let rgba = dynamic.to_rgba8();
            out.push(ImageBuffer {
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw(),
            });
        }
        Ok(out)
    }
}
