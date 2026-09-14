针对“将文档预览能力集成到 Bevy，并以插件形式提供”这一目标，以下是完整的库选取与 Cargo Features 设计方案。

---

## 📚 库的选取

### 纯 Rust 后端（默认，无 C 依赖）

| 格式 | 库 | 职责 | 关键能力 |
|:---|:---|:---|:---|
| **DOCX** | `rdocx` | 读取、布局、栅格化 | 内置布局引擎，可直接输出 PDF/PNG/HTML/Markdown，PNG 通过 `tiny-skia` 栅格化，支持 WASM |
| **XLSX** | `office2pdf` | Office 转 PDF | 纯 Rust，基于 Typst，支持 DOCX/XLSX/PPTX 转 PDF，无外部依赖，支持 WASM |
| **PDF** | `zpdf` | PDF 解析与栅格化 | 纯 Rust，CPU（`tiny-skia`）和 GPU（`wgpu`）双渲染后端，PNG 输出支持任意 DPI，输出匹配度 <1% 像素差异 |
| **PDF（备选）** | `hayro` | PDF 解析与栅格化 | 纯 Rust，支持 PDF 转 PNG/SVG，测试套件覆盖 1400+ PDF |
| **XLSX（数据提取）** | `calamine` | 电子表格数据读取 | 纯 Rust，只读，支持 XLS/XLSX/XLSM/XLSB/ODS |

> **`rdocx` 与 `office2pdf` 的分工**：`rdocx` 专注 DOCX 的深度处理（编辑、模板替换、合并等），其 PNG 输出是“DOCX 直出”路径；`office2pdf` 覆盖 DOCX/XLSX/PPTX 三种格式的统一转 PDF，是 XLSX 渲染的唯一纯 Rust 路径。两者可根据场景选择或组合使用。

### C 库后端（可选 feature）

| 格式 | 库 | 职责 | 依赖 |
|:---|:---|:---|:---|
| **全格式** | `libreoffice-rs` | LibreOfficeKit C API 的 Rust 封装 | 需系统安装 LibreOffice ≥ 6.0 + `libreofficekit-dev`，设置 `LO_INCLUDE_PATH` |

> **注意**：`libreoffice-rs` 的最新版本为 0.3.3，已约一年未更新，社区活跃度较低。将其作为可选 feature 而非默认依赖，是正确的工程决策。

### Bevy 集成

| 职责 | 库/方法 | 说明 |
|:---|:---|:---|
| 图像解码 | `image` crate | 将 PNG 字节流解码为 `DynamicImage` |
| 纹理转换 | `Image::from_dynamic()` | Bevy 内置方法，将 `DynamicImage` 转为 `Image` 资产 |
| 纹理采样 | `ImageSamplerDescriptor` | 配置 `Linear` 过滤 + Mipmap，保证缩放清晰度 |


## ⚙️ Cargo Features 设计

```toml
[package]
name = "bevy_doc_viewer"
version = "0.1.0"

[features]
# 默认启用纯 Rust 后端
default = ["pure-rust"]

# 纯 Rust 后端（无 C 依赖，跨平台，支持 WASM）
pure-rust = [
    "dep:rdocx",
    "dep:office2pdf",
    "dep:zpdf",
]

# LibreOfficeKit C 后端（高保真，需系统依赖）
libreoffice = ["dep:libreoffice-rs"]

# 调试辅助
debug-overlay = []

[dependencies]
# 公共依赖（始终启用）
bevy = { version = "0.14", default-features = false }
image = "0.25"

# 纯 Rust 后端依赖（可选）
rdocx = { version = "0.1", optional = true }
office2pdf = { version = "0.6", optional = true }
zpdf = { version = "0.4", optional = true }
calamine = { version = "0.36", optional = true }  # XLSX 数据提取，可选

# C 库后端依赖（可选）
libreoffice-rs = { version = "0.3", optional = true }
```

**Feature 组合矩阵**：

| `pure-rust` | `libreoffice` | 行为 |
|:---:|:---:|:---|
| ✅ | ❌ | 仅纯 Rust，默认，跨平台零依赖 |
| ❌ | ✅ | 仅 C 后端，体积小但需系统安装 LibreOffice |
| ✅ | ✅ | 两者都编入，运行时可选，`Auto` 优先纯 Rust |
| ❌ | ❌ | 编译错误，需在 crate 根部用 `compile_error!` 拦截 |


## 🧩 后端抽象层

核心是一个 trait，两种后端各自实现，使用者只面对这个 trait：

```rust
/// 文档栅格化后端统一接口
pub trait RasterBackend: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn supports(&self, format: DocFormat) -> bool;
    fn probe(&self, bytes: &[u8], format: DocFormat) -> Result<DocMetadata, DocError>;
    fn rasterize_page(
        &self, bytes: &[u8], format: DocFormat, page: usize, dpi: u32,
    ) -> Result<ImageBuffer, DocError>;
    fn shutdown(&self) {}
}
```

**纯 Rust 后端实现**：

```rust
#[cfg(feature = "pure-rust")]
impl RasterBackend for PureRustBackend {
    fn rasterize_page(&self, bytes, format, page, dpi) -> Result<ImageBuffer, DocError> {
        match format {
            DocFormat::Pdf => zpdf::render_page_to_image(bytes, page, dpi),
            DocFormat::Docx => {
                // 路径 A：rdocx 直出 PNG
                // 路径 B：office2pdf → PDF → zpdf
                let pdf = office2pdf::convert(bytes, DocFormat::Docx)?;
                zpdf::render_page_to_image(&pdf, page, dpi)
            }
            DocFormat::Xlsx => {
                let pdf = office2pdf::convert(bytes, DocFormat::Xlsx)?;
                zpdf::render_page_to_image(&pdf, page, dpi)
            }
        }
    }
}
```

**LibreOffice 后端实现**：

```rust
#[cfg(feature = "libreoffice")]
impl RasterBackend for LibreOfficeBackend {
    fn rasterize_page(&self, bytes, format, page, dpi) -> Result<ImageBuffer, DocError> {
        let doc = self.office.document_load_from_bytes(bytes, format)?;
        doc.render_page_png(page, dpi)
    }
}
```


## 💎 总结

**默认路径**（`cargo add bevy_doc_viewer`）：纯 Rust，零 C 依赖，跨平台，支持 WASM。DOCX 走 `rdocx` 或 `office2pdf` → `zpdf`，XLSX 走 `office2pdf` → `zpdf`，PDF 走 `zpdf`。适用于 90% 的常见文档场景。

**可选路径**（`--features libreoffice`）：系统安装 LibreOffice 后启用，获得近乎 100% 的 Office 文档兼容性，适合对保真度要求极高的场景。

**高级路径**（两个 feature 都启用）：运行时通过 `BackendPreference` 动态选择，支持降级策略——先用纯 Rust 快速渲染，遇到复杂文档再调用 C 后端。

这套设计将“依赖重量”转化为“用户可选项”，既保持了默认路径的纯净与跨平台能力，又为高保真需求提供了逃生舱，是工程上最稳健的权衡方案。
