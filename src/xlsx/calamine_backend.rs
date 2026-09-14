use super::backend::XlsxBackend;
use super::types::{XlsxError, XlsxMetadata, check_dpi, check_input};
use crate::common::ImageBuffer;
use calamine::{Reader, Xlsx};
use std::io::Cursor;

pub struct CalamineOfficeBackend;

impl CalamineOfficeBackend {
    fn workbook(xlsx: &[u8]) -> Result<Xlsx<Cursor<Vec<u8>>>, XlsxError> {
        check_input(xlsx)?;
        let cursor = Cursor::new(xlsx.to_vec());
        Xlsx::new(cursor).map_err(|e| XlsxError::Parse(format!("{e:?}")))
    }

    fn sheet_range(
        workbook: &mut Xlsx<Cursor<Vec<u8>>>,
        sheet: Option<&str>,
    ) -> Result<(String, calamine::Range<calamine::Data>), XlsxError> {
        let name = match sheet {
            Some(name) => {
                if !workbook.sheet_names().contains(&name.to_string()) {
                    return Err(XlsxError::SheetNotFound(name.to_string()));
                }
                name.to_string()
            }
            None => workbook
                .sheet_names()
                .first()
                .cloned()
                .ok_or_else(|| XlsxError::Parse("workbook has no sheets".to_string()))?,
        };
        let range = workbook
            .worksheet_range(&name)
            .map_err(|e| XlsxError::Parse(format!("{e:?}")))?;
        Ok((name, range))
    }

    fn pdf_pages(pdf: &[u8]) -> Result<usize, XlsxError> {
        #[cfg(feature = "pdf-hayro")]
        {
            crate::pdf::page_count(pdf).map_err(|e| XlsxError::Render(e.to_string()))
        }
        #[cfg(all(not(feature = "pdf-hayro"), feature = "pdf-zpdf"))]
        {
            crate::pdf::page_count(pdf).map_err(|e| XlsxError::Render(e.to_string()))
        }
        #[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
        {
            let _ = pdf;
            Err(XlsxError::NoBackend)
        }
    }

    fn rasterize_pdf(pdf: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, XlsxError> {
        check_dpi(dpi)?;
        #[cfg(any(feature = "pdf-hayro", feature = "pdf-zpdf"))]
        {
            crate::pdf::rasterize_page(pdf, page, dpi).map_err(|e| match e {
                crate::pdf::PdfError::PageOutOfRange { requested, total } => {
                    XlsxError::PageOutOfRange { requested, total }
                }
                crate::pdf::PdfError::InvalidDpi(dpi) => XlsxError::InvalidDpi(dpi),
                other => XlsxError::Render(other.to_string()),
            })
        }
        #[cfg(not(any(feature = "pdf-hayro", feature = "pdf-zpdf")))]
        {
            let _ = (pdf, page);
            Err(XlsxError::NoBackend)
        }
    }
}

impl XlsxBackend for CalamineOfficeBackend {
    fn name(&self) -> &'static str {
        "calamine-office2pdf"
    }

    fn probe(&self, xlsx: &[u8]) -> Result<XlsxMetadata, XlsxError> {
        let mut workbook = Self::workbook(xlsx)?;
        let sheets = workbook.sheet_names();
        let (_, range) = Self::sheet_range(&mut workbook, None)?;
        let (rows, cols) = range.get_size();
        let non_empty_cells = range
            .used_cells()
            .filter(|(_, _, cell)| *cell != &calamine::Data::Empty)
            .count();
        Ok(XlsxMetadata {
            sheets,
            rows,
            cols,
            non_empty_cells,
        })
    }

    fn sheet_names(&self, xlsx: &[u8]) -> Result<Vec<String>, XlsxError> {
        Ok(Self::workbook(xlsx)?.sheet_names())
    }

    fn extract_text(&self, xlsx: &[u8], sheet: Option<&str>) -> Result<String, XlsxError> {
        let mut workbook = Self::workbook(xlsx)?;
        let (_, range) = Self::sheet_range(&mut workbook, sheet)?;
        let mut lines = Vec::new();
        for row in range.rows() {
            let cells: Vec<String> = row.iter().map(cell_to_string).collect();
            let line = cells.join("\t").trim_end().to_string();
            if !line.trim().is_empty() {
                lines.push(line);
            }
        }
        Ok(lines.join("\n"))
    }

    fn to_pdf(&self, xlsx: &[u8]) -> Result<Vec<u8>, XlsxError> {
        check_input(xlsx)?;
        #[cfg(feature = "xlsx-office2pdf")]
        {
            office2pdf::convert_bytes(
                xlsx,
                office2pdf::config::Format::Xlsx,
                &office2pdf::config::ConvertOptions::default(),
            )
            .map(|result| result.pdf)
            .map_err(|e| XlsxError::Convert(format!("{e:?}")))
        }
        #[cfg(not(feature = "xlsx-office2pdf"))]
        {
            Err(XlsxError::NoBackend)
        }
    }

    fn page_count(&self, xlsx: &[u8]) -> Result<usize, XlsxError> {
        let pdf = self.to_pdf(xlsx)?;
        Self::pdf_pages(&pdf)
    }

    fn rasterize_page(&self, xlsx: &[u8], page: usize, dpi: u32) -> Result<ImageBuffer, XlsxError> {
        check_input(xlsx)?;
        check_dpi(dpi)?;
        let total = self.page_count(xlsx)?;
        if page >= total {
            return Err(XlsxError::PageOutOfRange {
                requested: page,
                total,
            });
        }
        let pdf = self.to_pdf(xlsx)?;
        Self::rasterize_pdf(&pdf, page, dpi)
    }

    fn rasterize_all(&self, xlsx: &[u8], dpi: u32) -> Result<Vec<ImageBuffer>, XlsxError> {
        check_input(xlsx)?;
        check_dpi(dpi)?;
        let pdf = self.to_pdf(xlsx)?;
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
            Err(XlsxError::NoBackend)
        }
    }
}

fn cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Empty => String::new(),
        calamine::Data::String(value) => value.clone(),
        calamine::Data::Int(value) => value.to_string(),
        calamine::Data::Float(value) => {
            if value.fract() == 0.0 && value.is_finite() {
                format!("{}", *value as i64)
            } else {
                value.to_string()
            }
        }
        calamine::Data::Bool(value) => value.to_string(),
        calamine::Data::DateTime(value) => value.to_string(),
        calamine::Data::DateTimeIso(value) => value.clone(),
        calamine::Data::DurationIso(value) => value.clone(),
        calamine::Data::Error(value) => format!("{value:?}"),
    }
}
