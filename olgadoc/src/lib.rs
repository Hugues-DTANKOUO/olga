// SPDX-License-Identifier: Apache-2.0

//! Python bindings for the Olga Intelligent Document Processing engine.
//!
//! `olgadoc` lets Python callers open PDF, DOCX, XLSX and HTML documents,
//! extract their text, markdown, images, links and tables, run full-text
//! search, and check a document's processability before committing to
//! downstream work.
//!
//! Build with [maturin](https://www.maturin.rs/) (`maturin develop`) and then:
//!
//! ```python
//! import olgadoc
//!
//! doc = olgadoc.Document.open("report.pdf")
//! print(doc.format, doc.page_count)
//! print(doc.text())
//!
//! for hit in doc.search("revenue"):
//!     print(hit["page"], hit["snippet"])
//!
//! report = doc.processability()
//! if report.is_blocked():
//!     raise SystemExit(report.blockers)
//! ```
//!
//! Every error raised by the engine surfaces as [`OlgaError`], and every
//! public class ships with full type stubs (`py.typed` + `olgadoc.pyi`).

use std::collections::BTreeMap;

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

use olga::api;
use olga::error::IdpError;

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

create_exception!(olgadoc, OlgaError, PyException);

fn to_py_err(e: IdpError) -> PyErr {
    OlgaError::new_err(e.to_string())
}

// ---------------------------------------------------------------------------
// Shared converters
// ---------------------------------------------------------------------------

fn format_to_string(format: api::Format) -> String {
    format.to_string()
}

fn image_format_label(format: olga::model::ImageFormat) -> &'static str {
    match format {
        olga::model::ImageFormat::Png => "png",
        olga::model::ImageFormat::Jpeg => "jpeg",
        olga::model::ImageFormat::Gif => "gif",
        olga::model::ImageFormat::Bmp => "bmp",
        olga::model::ImageFormat::Tiff => "tiff",
        olga::model::ImageFormat::Svg => "svg",
        olga::model::ImageFormat::Webp => "webp",
        olga::model::ImageFormat::Unknown => "unknown",
    }
}

fn bbox_dict<'py>(py: Python<'py>, bbox: &api::BoundingBox) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("x", bbox.x)?;
    d.set_item("y", bbox.y)?;
    d.set_item("width", bbox.width)?;
    d.set_item("height", bbox.height)?;
    Ok(d)
}

fn link_dict<'py>(py: Python<'py>, link: &api::Link) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("text", &link.text)?;
    d.set_item("url", &link.url)?;
    d.set_item("page", link.page)?;
    d.set_item("bbox", bbox_dict(py, &link.bbox)?)?;
    Ok(d)
}

fn table_cell_dict<'py>(py: Python<'py>, cell: &api::TableCell) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("row", cell.row)?;
    d.set_item("col", cell.col)?;
    d.set_item("rowspan", cell.rowspan)?;
    d.set_item("colspan", cell.colspan)?;
    d.set_item("text", &cell.text)?;
    Ok(d)
}

fn table_dict<'py>(py: Python<'py>, table: &api::Table) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("rows", table.rows)?;
    d.set_item("cols", table.cols)?;
    d.set_item("first_page", table.first_page)?;
    d.set_item("last_page", table.last_page)?;
    d.set_item("is_cross_page", table.is_cross_page())?;
    d.set_item("bbox", bbox_dict(py, &table.bbox)?)?;
    let cells = PyList::empty(py);
    for c in &table.cells {
        cells.append(table_cell_dict(py, c)?)?;
    }
    d.set_item("cells", cells)?;
    Ok(d)
}

fn search_hit_dict<'py>(py: Python<'py>, hit: &api::SearchHit) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("page", hit.page)?;
    d.set_item("line", hit.line)?;
    d.set_item("col_start", hit.col_start)?;
    d.set_item("match", &hit.match_text)?;
    d.set_item("snippet", &hit.snippet)?;
    Ok(d)
}

fn chunk_dict<'py>(py: Python<'py>, chunk: &api::Chunk) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("page", chunk.page)?;
    d.set_item("text", &chunk.text)?;
    d.set_item("char_count", chunk.char_count)?;
    Ok(d)
}

fn outline_dict<'py>(py: Python<'py>, entry: &api::OutlineEntry) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("level", entry.level)?;
    d.set_item("text", &entry.text)?;
    d.set_item("page", entry.page)?;
    d.set_item("bbox", bbox_dict(py, &entry.bbox)?)?;
    Ok(d)
}

fn image_dict<'py>(
    py: Python<'py>,
    image: &olga::model::ExtractedImage,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    // The internal field is 0-indexed; surface it as 1-based to match the
    // rest of the Python API.
    d.set_item("page", image.page.saturating_add(1))?;
    d.set_item("format", image_format_label(image.format))?;
    d.set_item("alt_text", image.alt_text.clone())?;
    d.set_item("bbox", bbox_dict(py, &image.bbox)?)?;
    d.set_item("data", PyBytes::new(py, &image.data))?;
    d.set_item("size", image.data.len())?;
    Ok(d)
}

/// Serialize a [`HealthIssue`] into a `{kind, ...}` dict so Python callers
/// can dispatch on `issue["kind"]` without inspecting enum discriminants.
fn health_issue_dict<'py>(
    py: Python<'py>,
    issue: &api::HealthIssue,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    match issue {
        api::HealthIssue::Encrypted => {
            d.set_item("kind", "Encrypted")?;
        }
        api::HealthIssue::EmptyContent => {
            d.set_item("kind", "EmptyContent")?;
        }
        api::HealthIssue::DecodeFailed => {
            d.set_item("kind", "DecodeFailed")?;
        }
        api::HealthIssue::ApproximatePagination { basis } => {
            d.set_item("kind", "ApproximatePagination")?;
            d.set_item("basis", basis.to_string())?;
        }
        api::HealthIssue::HeuristicStructure { pages } => {
            d.set_item("kind", "HeuristicStructure")?;
            d.set_item("pages", *pages)?;
        }
        api::HealthIssue::PartialExtraction { count } => {
            d.set_item("kind", "PartialExtraction")?;
            d.set_item("count", *count)?;
        }
        api::HealthIssue::MissingPart { count } => {
            d.set_item("kind", "MissingPart")?;
            d.set_item("count", *count)?;
        }
        api::HealthIssue::UnresolvedStyle { count } => {
            d.set_item("kind", "UnresolvedStyle")?;
            d.set_item("count", *count)?;
        }
        api::HealthIssue::UnresolvedRelationship { count } => {
            d.set_item("kind", "UnresolvedRelationship")?;
            d.set_item("count", *count)?;
        }
        api::HealthIssue::MissingMedia { count } => {
            d.set_item("kind", "MissingMedia")?;
            d.set_item("count", *count)?;
        }
        api::HealthIssue::TruncatedContent { count } => {
            d.set_item("kind", "TruncatedContent")?;
            d.set_item("count", *count)?;
        }
        api::HealthIssue::MalformedContent { count } => {
            d.set_item("kind", "MalformedContent")?;
            d.set_item("count", *count)?;
        }
        api::HealthIssue::FilteredArtifact { count } => {
            d.set_item("kind", "FilteredArtifact")?;
            d.set_item("count", *count)?;
        }
        api::HealthIssue::SuspectedArtifact { count } => {
            d.set_item("kind", "SuspectedArtifact")?;
            d.set_item("count", *count)?;
        }
        api::HealthIssue::OtherWarnings { count } => {
            d.set_item("kind", "OtherWarnings")?;
            d.set_item("count", *count)?;
        }
    }
    Ok(d)
}

fn health_label(h: api::Health) -> &'static str {
    match h {
        api::Health::Ok => "ok",
        api::Health::Degraded => "degraded",
        api::Health::Blocked => "blocked",
    }
}

fn text_by_page_dict<'py>(
    py: Python<'py>,
    map: &BTreeMap<usize, String>,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    for (k, v) in map {
        d.set_item(*k, v.clone())?;
    }
    Ok(d)
}

/// Round-trip a `serde_json::Value` into a Python object via the `json`
/// stdlib module. Keeps us free of extra conversion deps while still
/// producing idiomatic Python `dict` / `list` / scalars.
fn json_value_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
    let s = serde_json::to_string(value)
        .map_err(|e| PyRuntimeError::new_err(format!("serialize json: {e}")))?;
    let json_module = py.import("json")?;
    let obj = json_module.call_method1("loads", (s,))?;
    Ok(obj.unbind())
}

// ---------------------------------------------------------------------------
// Processability
// ---------------------------------------------------------------------------

/// Health report for an opened :class:`Document`.
///
/// A ``Processability`` instance tells you — before the rest of your
/// pipeline starts spending money — whether the document actually carries
/// native, extractable text, and how cleanly Olga can process it. It
/// distinguishes *blockers* (issues that stop processing outright — most
/// commonly ``EmptyContent`` on scanned PDFs that need OCR upstream, plus
/// ``Encrypted`` and ``DecodeFailed``) from *degradations* (issues that
/// still allow processing but reduce fidelity, like heuristic page
/// boundaries or partial style resolution).
///
/// Example:
///     >>> report = doc.processability()
///     >>> report.health
///     'degraded'
///     >>> report.is_processable
///     True
///     >>> [issue["kind"] for issue in report.degradations]
///     ['HeuristicStructure', 'PartialExtraction']
#[pyclass(module = "olgadoc", name = "Processability", skip_from_py_object)]
#[derive(Clone)]
struct PyProcessability {
    inner: api::Processability,
}

#[pymethods]
impl PyProcessability {
    /// The overall health verdict for the document.
    ///
    /// Returns:
    ///     One of ``"ok"``, ``"degraded"`` or ``"blocked"``.
    #[getter]
    fn health(&self) -> &'static str {
        health_label(self.inner.health)
    }

    /// Whether the document can be processed at all.
    ///
    /// Returns:
    ///     ``False`` only when the health is ``"blocked"``.
    #[getter]
    fn is_processable(&self) -> bool {
        self.inner.is_processable
    }

    /// Total number of pages detected in the document.
    ///
    /// Returns:
    ///     Page count, greater than or equal to zero.
    #[getter]
    fn pages_total(&self) -> u32 {
        self.inner.pages_total
    }

    /// Number of pages that carry non-empty text after extraction.
    ///
    /// Returns:
    ///     Page count, bounded above by :attr:`pages_total`.
    #[getter]
    fn pages_with_content(&self) -> u32 {
        self.inner.pages_with_content
    }

    /// Total number of decode and structure warnings emitted while loading.
    ///
    /// Returns:
    ///     Warning count. ``0`` for a clean document.
    #[getter]
    fn warning_count(&self) -> usize {
        self.inner.warning_count
    }

    /// Issues that prevent processing outright.
    ///
    /// Returns:
    ///     A list of ``{"kind": str, ...}`` dicts. Empty when the document
    ///     is processable.
    #[getter]
    fn blockers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for issue in &self.inner.blockers {
            list.append(health_issue_dict(py, issue)?)?;
        }
        Ok(list)
    }

    /// Issues that allow processing but reduce extraction fidelity.
    ///
    /// Returns:
    ///     A list of ``{"kind": str, ...}`` dicts describing each
    ///     degradation (heuristic pagination, partial extraction, unresolved
    ///     styles, and similar).
    #[getter]
    fn degradations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for issue in &self.inner.degradations {
            list.append(health_issue_dict(py, issue)?)?;
        }
        Ok(list)
    }

    /// Whether the document is fully processable with no degradations.
    ///
    /// Returns:
    ///     ``True`` when :attr:`health` is ``"ok"``.
    fn is_ok(&self) -> bool {
        self.inner.is_ok()
    }

    /// Whether the document is processable but has at least one degradation.
    ///
    /// Returns:
    ///     ``True`` when :attr:`health` is ``"degraded"``.
    fn is_degraded(&self) -> bool {
        self.inner.is_degraded()
    }

    /// Whether the document cannot be processed.
    ///
    /// Returns:
    ///     ``True`` when :attr:`health` is ``"blocked"``.
    fn is_blocked(&self) -> bool {
        self.inner.is_blocked()
    }

    fn __repr__(&self) -> String {
        format!(
            "Processability(health='{}', is_processable={}, blockers={}, degradations={}, pages_with_content={}/{}, warning_count={})",
            health_label(self.inner.health),
            self.inner.is_processable,
            self.inner.blockers.len(),
            self.inner.degradations.len(),
            self.inner.pages_with_content,
            self.inner.pages_total,
            self.inner.warning_count,
        )
    }
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

/// A single page inside a :class:`Document`.
///
/// Pages are 1-indexed. Obtain them via :meth:`Document.pages` (all pages)
/// or :meth:`Document.page` (a specific page number).
///
/// Example:
///     >>> page = doc.page(1)
///     >>> page.number
///     1
///     >>> page.text()[:40]
///     'Quarterly revenue report — 2024 Q4 ...'
#[pyclass(module = "olgadoc", name = "Page")]
struct PyPage {
    inner: api::Page,
}

#[pymethods]
impl PyPage {
    /// 1-based page number within the parent document.
    ///
    /// Returns:
    ///     An integer greater than or equal to 1.
    #[getter]
    fn number(&self) -> usize {
        self.inner.number()
    }

    /// Effective physical dimensions of the page, when available.
    ///
    /// PDF pages expose physical geometry; HTML and XLSX do not.
    ///
    /// Returns:
    ///     A dict with keys ``width_pt`` (float), ``height_pt`` (float) and
    ///     ``rotation`` (int, degrees) — or ``None`` if the format carries
    ///     no page geometry.
    #[getter]
    fn dimensions<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(d) = self.inner.dimensions() else {
            return Ok(None);
        };
        let out = PyDict::new(py);
        out.set_item("width_pt", d.effective_width)?;
        out.set_item("height_pt", d.effective_height)?;
        out.set_item("rotation", d.rotation)?;
        Ok(Some(out))
    }

    /// Plain-text rendering of the page.
    ///
    /// Returns:
    ///     The page's text as a UTF-8 string, potentially empty for blank
    ///     pages or image-only pages without OCR.
    fn text(&self) -> String {
        self.inner.text()
    }

    /// Markdown rendering of the page with headings and list structure.
    ///
    /// Returns:
    ///     The page's content as GitHub-flavoured markdown.
    fn markdown(&self) -> String {
        self.inner.markdown()
    }

    /// All raster images that live on this page.
    ///
    /// Returns:
    ///     A list of dicts carrying ``page``, ``format``, ``alt_text``,
    ///     ``bbox``, ``data`` (``bytes``) and ``size`` (``int``).
    fn images<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for img in self.inner.images() {
            list.append(image_dict(py, img)?)?;
        }
        Ok(list)
    }

    /// Number of images on this page.
    ///
    /// Returns:
    ///     Same as ``len(page.images())``, without materialising the list.
    fn image_count(&self) -> usize {
        self.inner.image_count()
    }

    /// Hyperlinks anchored on this page.
    ///
    /// Returns:
    ///     A list of dicts with keys ``text``, ``url``, ``page`` and
    ///     ``bbox``.
    fn links<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for link in self.inner.links() {
            list.append(link_dict(py, &link)?)?;
        }
        Ok(list)
    }

    /// Number of hyperlinks on this page.
    ///
    /// Returns:
    ///     Same as ``len(page.links())``, without materialising the list.
    fn link_count(&self) -> usize {
        self.inner.link_count()
    }

    /// Reconstructed tables whose first page is this one.
    ///
    /// Cross-page tables are anchored on their first page — check the
    /// ``is_cross_page`` key to detect them.
    ///
    /// Returns:
    ///     A list of dicts carrying ``rows``, ``cols``, ``first_page``,
    ///     ``last_page``, ``is_cross_page``, ``bbox`` and ``cells``.
    fn tables<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for table in self.inner.tables() {
            list.append(table_dict(py, &table)?)?;
        }
        Ok(list)
    }

    /// Number of tables anchored on this page.
    ///
    /// Returns:
    ///     Same as ``len(page.tables())``, without materialising the list.
    fn table_count(&self) -> usize {
        self.inner.table_count()
    }

    /// Search for a literal substring inside this page's text.
    ///
    /// The match is case-insensitive and substring-based.
    ///
    /// Args:
    ///     query: The text to look for. An empty string returns no hits.
    ///
    /// Returns:
    ///     A list of hit dicts with keys ``page``, ``line``, ``col_start``,
    ///     ``match`` and ``snippet``.
    fn search<'py>(&self, py: Python<'py>, query: &str) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for hit in self.inner.search(query) {
            list.append(search_hit_dict(py, &hit)?)?;
        }
        Ok(list)
    }

    /// The text chunk produced by the default chunker for this page.
    ///
    /// Returns:
    ///     A dict with ``page``, ``text`` and ``char_count`` — or ``None``
    ///     when the page is empty.
    fn chunk<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        match self.inner.chunk() {
            Some(c) => Ok(Some(chunk_dict(py, &c)?)),
            None => Ok(None),
        }
    }

    fn __repr__(&self) -> String {
        format!("Page(number={})", self.inner.number())
    }
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

/// An opened document — the primary entry point of the library.
///
/// Obtain an instance with :meth:`Document.open` (from a filesystem path)
/// or :meth:`Document.open_bytes` (from raw bytes). Once opened, the
/// document exposes its text, markdown, per-page content, images, links,
/// tables, search, outline, JSON tree, and a processability health report.
///
/// Example:
///     >>> doc = olgadoc.Document.open("report.pdf")
///     >>> doc.format, doc.page_count
///     ('PDF', 12)
///     >>> hit = doc.search("executive summary")[0]
///     >>> hit["page"], hit["snippet"]
///     (1, 'Executive summary: ...')
#[pyclass(module = "olgadoc", name = "Document")]
struct PyDocument {
    inner: api::Document,
}

#[pymethods]
impl PyDocument {
    /// Open a document from a filesystem path.
    ///
    /// Args:
    ///     path: Absolute or relative path to the document.
    ///
    /// Returns:
    ///     A fully loaded :class:`Document` ready for extraction.
    ///
    /// Raises:
    ///     OlgaError: If the file cannot be read, the format is
    ///         unsupported, the document is encrypted, or decoding fails.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        api::Document::open(path)
            .map(|d| Self { inner: d })
            .map_err(to_py_err)
    }

    /// Open a document from raw bytes already held in memory.
    ///
    /// Useful when the document arrives over HTTP or from a database blob.
    ///
    /// Args:
    ///     data: The raw bytes of the document.
    ///     format: Optional format hint — one of ``"pdf"``, ``"docx"``,
    ///         ``"docm"``, ``"xlsx"``, ``"xls"``, ``"html"`` or ``"htm"``.
    ///         When ``None``, the format is inferred from magic bytes.
    ///
    /// Returns:
    ///     A fully loaded :class:`Document`.
    ///
    /// Raises:
    ///     OlgaError: If the hint is unknown, the format is unsupported,
    ///         or decoding fails.
    #[staticmethod]
    #[pyo3(signature = (data, format=None))]
    fn open_bytes(data: Vec<u8>, format: Option<&str>) -> PyResult<Self> {
        let hint = match format {
            Some(s) => Some(parse_format_hint(s)?),
            None => None,
        };
        api::Document::open_bytes(data, hint)
            .map(|d| Self { inner: d })
            .map_err(to_py_err)
    }

    /// Document format as an uppercase label.
    ///
    /// Returns:
    ///     One of ``"PDF"``, ``"DOCX"``, ``"XLSX"`` or ``"HTML"``.
    #[getter]
    fn format(&self) -> String {
        format_to_string(self.inner.format())
    }

    /// Total number of pages in the document.
    ///
    /// Returns:
    ///     Page count, greater than or equal to zero.
    #[getter]
    fn page_count(&self) -> usize {
        self.inner.page_count()
    }

    /// Shortcut for ``doc.processability().is_processable``.
    ///
    /// Returns:
    ///     ``True`` unless the document is encrypted or otherwise blocked.
    #[getter]
    fn is_processable(&self) -> bool {
        self.inner.is_processable()
    }

    /// Document title from the underlying metadata, when provided.
    ///
    /// Returns:
    ///     The title as a string, or ``None`` if the format does not carry
    ///     one or the field is empty.
    #[getter]
    fn title(&self) -> Option<String> {
        self.inner.metadata().title.clone()
    }

    /// Size of the source document in bytes.
    ///
    /// Returns:
    ///     File size, greater than or equal to zero.
    #[getter]
    fn file_size(&self) -> u64 {
        self.inner.metadata().file_size
    }

    /// Whether the document is encrypted.
    ///
    /// Returns:
    ///     ``True`` when the source file is password-protected.
    #[getter]
    fn encrypted(&self) -> bool {
        self.inner.metadata().encrypted
    }

    /// Diagnostic warnings emitted during decoding and structure analysis.
    ///
    /// Returns:
    ///     A list of human-readable strings. Empty for a clean document.
    fn warnings(&self) -> Vec<String> {
        self.inner
            .warnings()
            .iter()
            .map(|w| w.to_string())
            .collect()
    }

    // ----------------------------------- Pages

    /// All pages in document order.
    ///
    /// Returns:
    ///     A list of :class:`Page` handles, one per page.
    fn pages<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for p in self.inner.pages() {
            list.append(Py::new(py, PyPage { inner: p })?)?;
        }
        Ok(list)
    }

    /// Fetch a specific page by its 1-based number.
    ///
    /// Args:
    ///     number: 1-based page index.
    ///
    /// Returns:
    ///     The :class:`Page` handle, or ``None`` if ``number`` is out of
    ///     range.
    fn page(&self, py: Python<'_>, number: usize) -> PyResult<Option<Py<PyPage>>> {
        match self.inner.page(number) {
            Some(p) => Ok(Some(Py::new(py, PyPage { inner: p })?)),
            None => Ok(None),
        }
    }

    // ----------------------------------- Text / Markdown

    /// Concatenated plain text of every page.
    ///
    /// Returns:
    ///     The whole document as a single UTF-8 string.
    fn text(&self) -> String {
        self.inner.text()
    }

    /// Concatenated markdown rendering of every page.
    ///
    /// Returns:
    ///     The whole document as GitHub-flavoured markdown.
    fn markdown(&self) -> String {
        self.inner.markdown()
    }

    /// Per-page plain text, keyed by 1-based page number.
    ///
    /// Returns:
    ///     A ``dict[int, str]`` mapping each page number to its text.
    fn text_by_page<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        text_by_page_dict(py, &self.inner.text_by_page())
    }

    /// Per-page markdown, keyed by 1-based page number.
    ///
    /// Returns:
    ///     A ``dict[int, str]`` mapping each page number to its markdown.
    fn markdown_by_page<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        text_by_page_dict(py, &self.inner.markdown_by_page())
    }

    // ----------------------------------- Images

    /// All raster images found in the document.
    ///
    /// Returns:
    ///     A list of dicts carrying ``page``, ``format``, ``alt_text``,
    ///     ``bbox``, ``data`` (``bytes``) and ``size`` (``int``).
    fn images<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for img in self.inner.images() {
            list.append(image_dict(py, img)?)?;
        }
        Ok(list)
    }

    /// Total number of images in the document.
    ///
    /// Returns:
    ///     Same as ``len(doc.images())``, without materialising the list.
    fn image_count(&self) -> usize {
        self.inner.image_count()
    }

    // ----------------------------------- Links

    /// All hyperlinks in the document.
    ///
    /// Returns:
    ///     A list of dicts with keys ``text``, ``url``, ``page`` and
    ///     ``bbox``.
    fn links<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for link in self.inner.links() {
            list.append(link_dict(py, &link)?)?;
        }
        Ok(list)
    }

    /// Total number of hyperlinks in the document.
    ///
    /// Returns:
    ///     Same as ``len(doc.links())``, without materialising the list.
    fn link_count(&self) -> usize {
        self.inner.link_count()
    }

    // ----------------------------------- Tables

    /// All reconstructed tables, including cross-page tables.
    ///
    /// Returns:
    ///     A list of dicts carrying ``rows``, ``cols``, ``first_page``,
    ///     ``last_page``, ``is_cross_page``, ``bbox`` and ``cells``.
    fn tables<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for table in self.inner.tables() {
            list.append(table_dict(py, &table)?)?;
        }
        Ok(list)
    }

    /// Total number of tables in the document.
    ///
    /// Returns:
    ///     Same as ``len(doc.tables())``, without materialising the list.
    fn table_count(&self) -> usize {
        self.inner.table_count()
    }

    // ----------------------------------- Search / Chunks

    /// Search for a literal substring across the full document.
    ///
    /// The match is case-insensitive and substring-based.
    ///
    /// Args:
    ///     query: The text to look for. An empty string returns no hits.
    ///
    /// Returns:
    ///     A list of hit dicts with keys ``page``, ``line``, ``col_start``,
    ///     ``match`` and ``snippet``.
    fn search<'py>(&self, py: Python<'py>, query: &str) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for hit in self.inner.search(query) {
            list.append(search_hit_dict(py, &hit)?)?;
        }
        Ok(list)
    }

    /// One text chunk per page, suitable for RAG-style indexing.
    ///
    /// Returns:
    ///     A list of dicts with keys ``page``, ``text`` and ``char_count``.
    fn chunks_by_page<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for chunk in self.inner.chunks_by_page() {
            list.append(chunk_dict(py, &chunk)?)?;
        }
        Ok(list)
    }

    // ----------------------------------- Structure

    /// Hierarchical outline (table of contents) of the document.
    ///
    /// Returns:
    ///     A list of dicts with keys ``level`` (1-based heading depth),
    ///     ``text``, ``page`` and ``bbox``.
    ///
    /// Raises:
    ///     OlgaError: If the outline cannot be computed.
    fn outline<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let entries = self.inner.outline().map_err(to_py_err)?;
        let list = PyList::empty(py);
        for e in &entries {
            list.append(outline_dict(py, e)?)?;
        }
        Ok(list)
    }

    /// Full document tree serialised into a JSON-compatible Python object.
    ///
    /// The result is a ``dict`` / ``list`` / scalar structure produced via
    /// :func:`json.loads`, so it is safe to re-serialise with
    /// :func:`json.dumps`.
    ///
    /// Returns:
    ///     A JSON-compatible Python object (typically a dict).
    ///
    /// Raises:
    ///     OlgaError: If serialisation fails.
    fn to_json(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.inner.to_json().map_err(to_py_err)?;
        json_value_to_py(py, &value)
    }

    // ----------------------------------- Processability

    /// Compute a health report for the document.
    ///
    /// Call this before paying for downstream work to know whether
    /// extraction is reliable, degraded, or outright blocked.
    ///
    /// Returns:
    ///     A :class:`Processability` instance describing blockers and
    ///     degradations.
    fn processability(&self) -> PyProcessability {
        PyProcessability {
            inner: self.inner.processability(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Document(format='{}', page_count={}, encrypted={}, title={:?})",
            format_to_string(self.inner.format()),
            self.inner.page_count(),
            self.inner.metadata().encrypted,
            self.inner.metadata().title,
        )
    }
}

fn parse_format_hint(s: &str) -> PyResult<api::Format> {
    match s.to_ascii_lowercase().as_str() {
        "pdf" => Ok(api::Format::Pdf),
        "docx" | "docm" => Ok(api::Format::Docx),
        "xlsx" | "xls" => Ok(api::Format::Xlsx),
        "html" | "htm" => Ok(api::Format::Html),
        other => Err(OlgaError::new_err(format!(
            "unknown format hint '{other}' (expected: pdf, docx, xlsx, html)"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Module init
// ---------------------------------------------------------------------------

#[pymodule]
fn olgadoc(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("OlgaError", py.get_type::<OlgaError>())?;
    m.add_class::<PyDocument>()?;
    m.add_class::<PyPage>()?;
    m.add_class::<PyProcessability>()?;
    Ok(())
}
