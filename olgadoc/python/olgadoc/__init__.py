"""Python bindings for Olga — Intelligent Document Processing.

``olgadoc`` opens PDF, DOCX, XLSX, and HTML documents and exposes their text,
markdown, per-page content, images, links, tables, full-text search, and a
processability health report.

Every dict payload returned by the library is declared below as a
:class:`~typing.TypedDict` — both for ``mypy`` / ``pyright`` users and for
runtime introspection (``from olgadoc import Link; Link.__annotations__``).

Example:
    >>> import olgadoc
    >>> doc = olgadoc.Document.open("report.pdf")
    >>> doc.format, doc.page_count
    ('PDF', 2)
    >>> print(doc.text()[:200])
    >>> report = doc.processability()
    >>> report.health
    'ok'
"""

from __future__ import annotations

from typing import List, Literal, Optional, TypedDict, Union

# ``Required`` landed in :mod:`typing` in Python 3.11; the
# ``typing_extensions`` backport covers 3.8–3.10 and is treated as
# interchangeable by every mainstream type checker.
from typing_extensions import Required

from .olgadoc import (
    Document as Document,
)
from .olgadoc import (
    OlgaError as OlgaError,
)
from .olgadoc import (
    Page as Page,
)
from .olgadoc import (
    Processability as Processability,
)
from .olgadoc import (
    __version__ as __version__,
)

# ---------------------------------------------------------------------------
# Enumerated literals
# ---------------------------------------------------------------------------

HealthLabel = Literal["ok", "degraded", "blocked"]
FormatName = Literal["PDF", "DOCX", "XLSX", "HTML"]
FormatHint = Literal["pdf", "docx", "docm", "xlsx", "xls", "html", "htm"]

HealthIssueKind = Literal[
    "Encrypted",
    "EmptyContent",
    "DecodeFailed",
    "ApproximatePagination",
    "HeuristicStructure",
    "PartialExtraction",
    "MissingPart",
    "UnresolvedStyle",
    "UnresolvedRelationship",
    "MissingMedia",
    "TruncatedContent",
    "MalformedContent",
    "FilteredArtifact",
    "SuspectedArtifact",
    "OtherWarnings",
]

JsonElementType = Literal[
    "document",
    "section",
    "heading",
    "paragraph",
    "table",
    "table_row",
    "table_cell",
    "list",
    "list_item",
    "image",
    "code_block",
    "block_quote",
    "page_header",
    "page_footer",
    "footnote",
    "aligned_line",
]

# ---------------------------------------------------------------------------
# Shared dict payloads
# ---------------------------------------------------------------------------


class BoundingBox(TypedDict, total=False):
    """Axis-aligned bounding box in page coordinates (points)."""

    x: Required[float]
    y: Required[float]
    width: Required[float]
    height: Required[float]


class PageDimensions(TypedDict, total=False):
    """Effective page dimensions returned by :attr:`Page.dimensions`."""

    width_pt: Required[float]
    height_pt: Required[float]
    rotation: Required[int]


class Link(TypedDict, total=False):
    """A hyperlink extracted from a document."""

    text: Required[str]
    url: Required[str]
    page: Required[int]
    bbox: Required[BoundingBox]


class TableCell(TypedDict, total=False):
    """A single cell inside a :class:`Table`."""

    row: Required[int]
    col: Required[int]
    rowspan: Required[int]
    colspan: Required[int]
    text: Required[str]


class Table(TypedDict, total=False):
    """A reconstructed table, potentially spanning multiple pages."""

    rows: Required[int]
    cols: Required[int]
    first_page: Required[int]
    last_page: Required[int]
    is_cross_page: Required[bool]
    bbox: Required[BoundingBox]
    cells: Required[List[TableCell]]


class SearchHit(TypedDict, total=False):
    """A single match returned by :meth:`Document.search` or :meth:`Page.search`."""

    page: Required[int]
    line: Required[int]
    col_start: Required[int]
    # ``match`` is a soft keyword (Python 3.10 pattern matching) but remains
    # a valid dict key — keep the literal name for fidelity with the engine.
    match: Required[str]
    snippet: Required[str]


class Chunk(TypedDict, total=False):
    """A per-page text chunk returned by :meth:`Document.chunks_by_page`."""

    page: Required[int]
    text: Required[str]
    char_count: Required[int]


class OutlineEntry(TypedDict, total=False):
    """A single heading in :meth:`Document.outline`."""

    level: Required[int]
    text: Required[str]
    page: Required[int]
    bbox: Required[BoundingBox]


class ExtractedImage(TypedDict, total=False):
    """A raster image extracted from a document.

    ``alt_text`` is required to be present as a key but may be ``None`` when
    the source format does not carry alt text.
    """

    page: Required[int]
    format: Required[str]
    alt_text: Required[Optional[str]]
    bbox: Required[BoundingBox]
    data: Required[bytes]
    size: Required[int]


# ---------------------------------------------------------------------------
# Processability health issues
# ---------------------------------------------------------------------------


class HealthIssueSimple(TypedDict, total=False):
    """Health issue variants that carry no extra payload."""

    kind: Required[Literal["Encrypted", "EmptyContent", "DecodeFailed"]]


class HealthIssueApproximatePagination(TypedDict, total=False):
    """Pages are approximate — the engine inferred boundaries heuristically."""

    kind: Required[Literal["ApproximatePagination"]]
    basis: Required[str]


class HealthIssueHeuristicStructure(TypedDict, total=False):
    """Document structure was reconstructed heuristically on ``pages`` pages."""

    kind: Required[Literal["HeuristicStructure"]]
    pages: Required[int]


class HealthIssueCounted(TypedDict, total=False):
    """Every health variant that carries an occurrence ``count``."""

    kind: Required[
        Literal[
            "PartialExtraction",
            "MissingPart",
            "UnresolvedStyle",
            "UnresolvedRelationship",
            "MissingMedia",
            "TruncatedContent",
            "MalformedContent",
            "FilteredArtifact",
            "SuspectedArtifact",
            "OtherWarnings",
        ]
    ]
    count: Required[int]


HealthIssue = Union[
    HealthIssueSimple,
    HealthIssueApproximatePagination,
    HealthIssueHeuristicStructure,
    HealthIssueCounted,
]

# ---------------------------------------------------------------------------
# Structured JSON tree (returned by :meth:`Document.to_json`)
# ---------------------------------------------------------------------------
#
# Bounding boxes inside the JSON tree use the short-form ``{x, y, w, h}``
# encoding (normalized 0–1 page coordinates). This is deliberately distinct
# from :class:`BoundingBox` (used by :meth:`Document.links`,
# :meth:`Document.tables`, …) which carries ``{x, y, width, height}`` in
# points.
#
# ``confidence`` and ``source`` are stripped when they equal their defaults
# (``1.0`` and ``"format-derived"``) to keep the JSON compact — both keys are
# therefore optional on every element.


class JsonBBox(TypedDict, total=False):
    """Bounding box in normalized 0–1 page coordinates inside the JSON tree."""

    x: Required[float]
    y: Required[float]
    w: Required[float]
    h: Required[float]


class JsonSource(TypedDict, total=False):
    """Document-level metadata block inside the JSON tree."""

    format: Required[str]
    page_count: Required[int]
    file_size: Required[int]
    title: Required[Optional[str]]
    encrypted: Required[bool]


class JsonPageInfo(TypedDict, total=False):
    """Per-page geometry entry inside the JSON tree's ``pages`` array."""

    page: Required[int]
    width_pt: Required[float]
    height_pt: Required[float]
    rotation: Required[int]


class JsonWarning(TypedDict, total=False):
    """Decoder or structuring warning embedded in the JSON tree."""

    kind: Required[str]
    message: Required[str]
    page: Required[Optional[int]]


class JsonSpan(TypedDict, total=False):
    """A formatted run inside a :class:`JsonAlignedLineElement`.

    ``bold``, ``italic`` and ``link`` are only emitted when at least one
    span on the line carries formatting — a plain paragraph line omits the
    ``spans`` array entirely.
    """

    col: Required[int]
    text: Required[str]
    bold: bool
    italic: bool
    link: str


class JsonTableCellDetail(TypedDict, total=False):
    """Detailed cell inside a :class:`JsonTableElement` ``cells`` array.

    The ``cells`` array is only emitted when at least one cell has a
    non-trivial ``rowspan`` or ``colspan``. ``rowspan``, ``colspan`` and
    ``is_header`` are only present when they depart from their defaults.
    """

    id: Required[str]
    row: Required[int]
    col: Required[int]
    text: Required[str]
    bbox: Required[JsonBBox]
    rowspan: int
    colspan: int
    is_header: bool


# -- Per-variant element shapes ---------------------------------------------


class JsonDocumentElement(TypedDict, total=False):
    """Root ``document`` element in the JSON tree."""

    id: Required[str]
    type: Required[Literal["document"]]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonSectionElement(TypedDict, total=False):
    """A ``section`` element (logical grouping under a heading)."""

    id: Required[str]
    type: Required[Literal["section"]]
    level: Required[int]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonHeadingElement(TypedDict, total=False):
    """A ``heading`` element with its text and level."""

    id: Required[str]
    type: Required[Literal["heading"]]
    level: Required[int]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonParagraphElement(TypedDict, total=False):
    """A ``paragraph`` element with its text content."""

    id: Required[str]
    type: Required[Literal["paragraph"]]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonTableElement(TypedDict, total=False):
    """A ``table`` element with headers, data grid, and optional detailed cells."""

    id: Required[str]
    type: Required[Literal["table"]]
    rows: Required[int]
    cols: Required[int]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    headers: Required[List[str]]
    data: Required[List[List[str]]]
    cells: List[JsonTableCellDetail]
    children: List["JsonElement"]


class JsonTableRowElement(TypedDict, total=False):
    """A ``table_row`` element — normally inlined into its parent table."""

    id: Required[str]
    type: Required[Literal["table_row"]]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonTableCellElement(TypedDict, total=False):
    """A ``table_cell`` element with its row/column position and text."""

    id: Required[str]
    type: Required[Literal["table_cell"]]
    row: Required[int]
    col: Required[int]
    rowspan: Required[int]
    colspan: Required[int]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonListElement(TypedDict, total=False):
    """A ``list`` element (``ordered`` distinguishes ordered from bulleted)."""

    id: Required[str]
    type: Required[Literal["list"]]
    ordered: Required[bool]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonListItemElement(TypedDict, total=False):
    """A ``list_item`` element with its text content."""

    id: Required[str]
    type: Required[Literal["list_item"]]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonImageElement(TypedDict, total=False):
    """An ``image`` element with its MIME ``format`` and optional alt text."""

    id: Required[str]
    type: Required[Literal["image"]]
    format: Required[str]
    alt_text: Required[Optional[str]]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonCodeBlockElement(TypedDict, total=False):
    """A ``code_block`` element with optional ``language`` tag."""

    id: Required[str]
    type: Required[Literal["code_block"]]
    language: Required[Optional[str]]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonBlockQuoteElement(TypedDict, total=False):
    """A ``block_quote`` element with its text content."""

    id: Required[str]
    type: Required[Literal["block_quote"]]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonPageHeaderElement(TypedDict, total=False):
    """A ``page_header`` element — recurring header text on a page."""

    id: Required[str]
    type: Required[Literal["page_header"]]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonPageFooterElement(TypedDict, total=False):
    """A ``page_footer`` element — recurring footer text on a page."""

    id: Required[str]
    type: Required[Literal["page_footer"]]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonFootnoteElement(TypedDict, total=False):
    """A ``footnote`` element carrying its identifier and text."""

    id: Required[str]
    type: Required[Literal["footnote"]]
    footnote_id: Required[str]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    children: List["JsonElement"]


class JsonAlignedLineElement(TypedDict, total=False):
    """A layout-preserving line with optional per-span formatting."""

    id: Required[str]
    type: Required[Literal["aligned_line"]]
    text: Required[str]
    bbox: Required[JsonBBox]
    page: Required[int]
    confidence: float
    source: str
    spans: List[JsonSpan]
    children: List["JsonElement"]


JsonElement = Union[
    JsonDocumentElement,
    JsonSectionElement,
    JsonHeadingElement,
    JsonParagraphElement,
    JsonTableElement,
    JsonTableRowElement,
    JsonTableCellElement,
    JsonListElement,
    JsonListItemElement,
    JsonImageElement,
    JsonCodeBlockElement,
    JsonBlockQuoteElement,
    JsonPageHeaderElement,
    JsonPageFooterElement,
    JsonFootnoteElement,
    JsonAlignedLineElement,
]


class DocumentJson(TypedDict, total=False):
    """Full structured JSON tree returned by :meth:`Document.to_json`.

    The tree carries every structural element the engine resolved, along
    with document-level metadata, per-page geometry, and (when relevant)
    decoder warnings. The payload round-trips cleanly through
    :func:`json.dumps` / :func:`json.loads`.
    """

    olga_version: Required[str]
    source: Required[JsonSource]
    pages: Required[List[JsonPageInfo]]
    elements: Required[List[JsonElement]]
    warnings: List[JsonWarning]


__all__ = [
    "BoundingBox",
    "Chunk",
    "Document",
    "DocumentJson",
    "ExtractedImage",
    "FormatHint",
    "FormatName",
    "HealthIssue",
    "HealthIssueApproximatePagination",
    "HealthIssueCounted",
    "HealthIssueHeuristicStructure",
    "HealthIssueKind",
    "HealthIssueSimple",
    "HealthLabel",
    "JsonAlignedLineElement",
    "JsonBBox",
    "JsonBlockQuoteElement",
    "JsonCodeBlockElement",
    "JsonDocumentElement",
    "JsonElement",
    "JsonElementType",
    "JsonFootnoteElement",
    "JsonHeadingElement",
    "JsonImageElement",
    "JsonListElement",
    "JsonListItemElement",
    "JsonPageFooterElement",
    "JsonPageHeaderElement",
    "JsonPageInfo",
    "JsonParagraphElement",
    "JsonSectionElement",
    "JsonSource",
    "JsonSpan",
    "JsonTableCellDetail",
    "JsonTableCellElement",
    "JsonTableElement",
    "JsonTableRowElement",
    "JsonWarning",
    "Link",
    "OlgaError",
    "OutlineEntry",
    "Page",
    "PageDimensions",
    "Processability",
    "SearchHit",
    "Table",
    "TableCell",
    "__version__",
]
