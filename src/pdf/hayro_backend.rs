use super::backend::RasterBackend;
use super::types::{DocMetadata, ImageBuffer, PdfError, check_input, check_page, dpi_to_scale};

pub struct HayroBackend;

impl RasterBackend for HayroBackend {
    fn name(&self) -> &'static str {
        "hayro"
    }

    fn page_count(&self, pdf: &[u8]) -> Result<usize, PdfError> {
        check_input(pdf)?;
        let doc = hayro::hayro_syntax::Pdf::new(pdf.to_vec())
            .map_err(|e| PdfError::Parse(format!("{e:?}")))?;
        Ok(doc.pages().len())
    }

    fn probe(&self, pdf: &[u8]) -> Result<DocMetadata, PdfError> {
        check_input(pdf)?;
        let doc = hayro::hayro_syntax::Pdf::new(pdf.to_vec())
            .map_err(|e| PdfError::Parse(format!("{e:?}")))?;
        Ok(DocMetadata {
            pages: doc.pages().len(),
            version: format!("{:?}", doc.version()),
        })
    }

    fn rasterize_page(&self, pdf: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, PdfError> {
        check_input(pdf)?;
        let scale = dpi_to_scale(dpi)?;
        let doc = hayro::hayro_syntax::Pdf::new(pdf.to_vec())
            .map_err(|e| PdfError::Parse(format!("{e:?}")))?;
        let pages = doc.pages();
        check_page(page, pages.len())?;
        let pdf_page = pages.get(page).ok_or(PdfError::PageOutOfRange {
            requested: page,
            total: pages.len(),
        })?;
        let cache = hayro::RenderCache::new();
        let settings = hayro::RenderSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: hayro::vello_cpu::color::palette::css::WHITE,
            ..Default::default()
        };
        let pixmap = hayro::render(pdf_page, &cache, &Default::default(), &settings);
        let width = pixmap.width() as u32;
        let height = pixmap.height() as u32;
        let png = pixmap
            .into_png()
            .map_err(|e| PdfError::Render(e.to_string()))?;
        let dynamic = image::load_from_memory(&png).map_err(|e| PdfError::Image(e.to_string()))?;
        Ok(ImageBuffer {
            width,
            height,
            rgba: dynamic.to_rgba8().into_raw(),
        })
    }
}
