use super::backend::RasterBackend;
use super::types::{DocMetadata, ImageBuffer, PdfError, check_input, check_page, dpi_to_scale};
use zpdf::RenderBackend as _;

pub struct ZpdfBackend;

impl RasterBackend for ZpdfBackend {
    fn name(&self) -> &'static str {
        "zpdf"
    }

    fn page_count(&self, pdf: &[u8]) -> Result<usize, PdfError> {
        check_input(pdf)?;
        let doc =
            zpdf::PdfDocument::open(pdf.to_vec()).map_err(|e| PdfError::Parse(e.to_string()))?;
        Ok(doc.page_count())
    }

    fn probe(&self, pdf: &[u8]) -> Result<DocMetadata, PdfError> {
        check_input(pdf)?;
        let doc =
            zpdf::PdfDocument::open(pdf.to_vec()).map_err(|e| PdfError::Parse(e.to_string()))?;
        let (major, minor) = doc.version();
        Ok(DocMetadata {
            pages: doc.page_count(),
            version: format!("{major}.{minor}"),
        })
    }

    fn rasterize_page(&self, pdf: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, PdfError> {
        check_input(pdf)?;
        let scale = dpi_to_scale(dpi)?;
        let doc =
            zpdf::PdfDocument::open(pdf.to_vec()).map_err(|e| PdfError::Parse(e.to_string()))?;
        check_page(page, doc.page_count())?;
        let pdf_page = doc.page(page).map_err(|e| PdfError::Parse(e.to_string()))?;
        let mut fonts = doc.load_page_fonts(&pdf_page);
        let mut images = zpdf::ImageCache::new();
        let content = doc
            .page_content_bytes(&pdf_page)
            .map_err(|e| PdfError::Parse(e.to_string()))?;
        let display_list = zpdf::ContentInterpreter::new(pdf_page.effective_box())
            .with_page_rotation(pdf_page.rotate)
            .with_fonts(&mut fonts)
            .with_document(doc.file(), &pdf_page.resources)
            .with_images(&mut images)
            .interpret(&content);
        let mut renderer = zpdf::cpu::CpuRenderer::new()
            .with_fonts(&fonts)
            .with_images(&images);
        let rendered = renderer
            .render_display_list(&display_list, scale)
            .map_err(|e| PdfError::Render(e.to_string()))?;
        Ok(ImageBuffer {
            width: rendered.width,
            height: rendered.height,
            rgba: rendered.data,
        })
    }
}
