"""Type stubs for the ``olgadoc`` Rust extension module.

This stub describes the four compiled classes exposed by the Rust layer —
:class:`Document`, :class:`Page`, :class:`Processability` and
:class:`OlgaError` — plus the package ``__version__`` string. Every dict
payload type (``Link``, ``Table``, ``SearchHit``, ``DocumentJson`` and the
full ``JsonElement`` discriminated union) is declared as a real runtime
:class:`TypedDict` in :mod:`olgadoc.__init__` and referenced here, so IDEs,
``mypy`` and ``pyright`` offer autocompletion and reject malformed payloads.
"""

from __future__ import annotations

from olgadoc import (
    Chunk,
    DocumentJson,
    ExtractedImage,
    FormatHint,
    FormatName,
    HealthIssue,
    HealthLabel,
    Link,
    OutlineEntry,
    PageDimensions,
    SearchHit,
    Table,
)

__version__: str

# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------

class OlgaError(Exception):
    """Raised for every error surfaced by the Olga engine.

    The exception message reflects the underlying Rust error variant —
    encrypted document, unsupported format, decode failure, and so on — so
    callers can pattern-match on substrings or simply re-raise.
    """

# ---------------------------------------------------------------------------
# Classes
# ---------------------------------------------------------------------------

class Processability:
    """Health report for an opened :class:`Document`.

    A ``Processability`` instance tells you — before the rest of your
    pipeline starts spending money — whether the document actually
    carries native, extractable text, and how cleanly Olga can process
    it. It distinguishes *blockers* (issues that stop processing outright
    — most commonly ``EmptyContent`` on scanned PDFs that need OCR
    upstream, plus ``Encrypted`` and ``DecodeFailed``) from
    *degradations* (issues that still allow processing but reduce
    fidelity).

    Example:
        ```python
        report = doc.processability()
        report.health              # -> 'degraded'
        report.is_processable      # -> True
        [i["kind"] for i in report.degradations]
        # -> HeuristicStructure, PartialExtraction
        ```
    """

    @property
    def health(self) -> HealthLabel:
        """The overall health verdict.

        Returns:
            One of ``"ok"``, ``"degraded"`` or ``"blocked"``.
        """

    @property
    def is_processable(self) -> bool:
        """Whether the document can be processed at all.

        Returns:
            ``False`` only when :attr:`health` is ``"blocked"``.
        """

    @property
    def pages_total(self) -> int:
        """Total number of pages detected in the document.

        Returns:
            Page count, greater than or equal to zero.
        """

    @property
    def pages_with_content(self) -> int:
        """Number of pages that carry non-empty text after extraction.

        Returns:
            Page count, bounded above by :attr:`pages_total`.
        """

    @property
    def warning_count(self) -> int:
        """Total number of warnings emitted while loading the document.

        Returns:
            Warning count. ``0`` for a clean document.
        """

    @property
    def blockers(self) -> list[HealthIssue]:
        """Issues that prevent processing outright.

        Returns:
            A list of ``{"kind": str, ...}`` dicts. Empty when the document
            is processable.
        """

    @property
    def degradations(self) -> list[HealthIssue]:
        """Issues that allow processing but reduce extraction fidelity.

        Returns:
            A list of ``{"kind": str, ...}`` dicts.
        """

    def is_ok(self) -> bool:
        """Whether the document is fully processable with no degradations.

        Returns:
            ``True`` when :attr:`health` is ``"ok"``.
        """

    def is_degraded(self) -> bool:
        """Whether the document is processable but has at least one degradation.

        Returns:
            ``True`` when :attr:`health` is ``"degraded"``.
        """

    def is_blocked(self) -> bool:
        """Whether the document cannot be processed.

        Returns:
            ``True`` when :attr:`health` is ``"blocked"``.
        """

    def __repr__(self) -> str: ...

class Page:
    """A single page inside a :class:`Document`.

    Pages are 1-indexed. Obtain them via :meth:`Document.pages` (all pages)
    or :meth:`Document.page` (a specific page number).

    Example:
        >>> page = doc.page(1)
        >>> page.number
        1
        >>> page.text()[:40]
        'Quarterly revenue report — 2024 Q4 ...'
    """

    @property
    def number(self) -> int:
        """1-based page number within the parent document.

        Returns:
            An integer greater than or equal to 1.
        """

    @property
    def dimensions(self) -> PageDimensions | None:
        """Effective physical dimensions of the page, when available.

        PDF pages expose physical geometry; HTML and XLSX do not.

        Returns:
            A :class:`PageDimensions` dict, or ``None`` if the format does
            not carry page geometry.
        """

    def text(self) -> str:
        """Plain-text rendering of the page.

        Returns:
            The page's text as a UTF-8 string, potentially empty for blank
            pages or image-only pages without OCR.
        """

    def markdown(self) -> str:
        """Markdown rendering of the page with headings and list structure.

        Returns:
            The page's content as GitHub-flavoured markdown.
        """

    def images(self) -> list[ExtractedImage]:
        """Every raster image that lives on this page.

        Returns:
            A list of :class:`ExtractedImage` dicts.
        """

    def image_count(self) -> int:
        """Number of images on this page.

        Returns:
            Same as ``len(page.images())``, without materialising the list.
        """

    def links(self) -> list[Link]:
        """Hyperlinks anchored on this page.

        Returns:
            A list of :class:`Link` dicts.
        """

    def link_count(self) -> int:
        """Number of hyperlinks on this page.

        Returns:
            Same as ``len(page.links())``, without materialising the list.
        """

    def tables(self) -> list[Table]:
        """Reconstructed tables whose first page is this one.

        Cross-page tables are anchored on their first page — inspect the
        ``is_cross_page`` key to detect them.

        Returns:
            A list of :class:`Table` dicts.
        """

    def table_count(self) -> int:
        """Number of tables anchored on this page.

        Returns:
            Same as ``len(page.tables())``, without materialising the list.
        """

    def search(self, query: str) -> list[SearchHit]:
        """Search for a literal substring inside this page's text.

        The match is case-insensitive and substring-based.

        Args:
            query: The text to look for. An empty string returns no hits.

        Returns:
            A list of :class:`SearchHit` dicts.
        """

    def chunk(self) -> Chunk | None:
        """Text chunk produced by the default chunker for this page.

        Returns:
            A :class:`Chunk` dict, or ``None`` when the page is empty.
        """

    def __repr__(self) -> str: ...

class Document:
    """An opened document — the primary entry point of the library.

    Obtain an instance with :meth:`Document.open` (from a filesystem path)
    or :meth:`Document.open_bytes` (from raw bytes). Once opened, the
    document exposes its text, markdown, per-page content, images, links,
    tables, full-text search, outline, JSON tree, and a processability
    health report.

    Example:
        >>> doc = olgadoc.Document.open("report.pdf")
        >>> doc.format, doc.page_count
        ('PDF', 12)
        >>> hit = doc.search("executive summary")[0]
        >>> hit["page"], hit["snippet"]
        (1, 'Executive summary: ...')
    """

    @staticmethod
    def open(path: str) -> Document:
        """Open a document from a filesystem path.

        Args:
            path: Absolute or relative path to the document.

        Returns:
            A fully loaded :class:`Document` ready for extraction.

        Raises:
            OlgaError: If the file cannot be read, the format is
                unsupported, the document is encrypted, or decoding fails.
        """

    @staticmethod
    def open_bytes(data: bytes, format: FormatHint | None = ...) -> Document:
        """Open a document from raw bytes already held in memory.

        Useful when the document arrives over HTTP or from a database blob.

        Args:
            data: The raw bytes of the document.
            format: Optional format hint. When ``None``, the format is
                inferred from magic bytes.

        Returns:
            A fully loaded :class:`Document`.

        Raises:
            OlgaError: If the hint is unknown, the format is unsupported,
                or decoding fails.
        """

    @property
    def format(self) -> FormatName:
        """Document format as an uppercase label.

        Returns:
            One of ``"PDF"``, ``"DOCX"``, ``"XLSX"`` or ``"HTML"``.
        """

    @property
    def page_count(self) -> int:
        """Total number of pages in the document.

        Returns:
            Page count, greater than or equal to zero.
        """

    @property
    def is_processable(self) -> bool:
        """Shortcut for ``doc.processability().is_processable``.

        Returns:
            ``True`` unless the document is encrypted or otherwise blocked.
        """

    @property
    def title(self) -> str | None:
        """Document title from the underlying metadata, when provided.

        Returns:
            The title as a string, or ``None`` if absent.
        """

    @property
    def file_size(self) -> int:
        """Size of the source document in bytes.

        Returns:
            File size, greater than or equal to zero.
        """

    @property
    def encrypted(self) -> bool:
        """Whether the document is encrypted.

        Returns:
            ``True`` when the source file is password-protected.
        """

    def warnings(self) -> list[str]:
        """Diagnostic warnings emitted during decoding and structure analysis.

        Returns:
            A list of human-readable strings. Empty for a clean document.
        """

    def pages(self) -> list[Page]:
        """All pages in document order.

        Returns:
            A list of :class:`Page` handles, one per page.
        """

    def page(self, number: int) -> Page | None:
        """Fetch a specific page by its 1-based number.

        Args:
            number: 1-based page index.

        Returns:
            The :class:`Page` handle, or ``None`` if ``number`` is out of
            range.
        """

    def text(self) -> str:
        """Concatenated plain text of every page.

        Returns:
            The whole document as a single UTF-8 string.
        """

    def markdown(self) -> str:
        """Concatenated markdown rendering of every page.

        Returns:
            The whole document as GitHub-flavoured markdown.
        """

    def text_by_page(self) -> dict[int, str]:
        """Per-page plain text, keyed by 1-based page number.

        Returns:
            A dict mapping each page number to its text.
        """

    def markdown_by_page(self) -> dict[int, str]:
        """Per-page markdown, keyed by 1-based page number.

        Returns:
            A dict mapping each page number to its markdown.
        """

    def images(self) -> list[ExtractedImage]:
        """All raster images found in the document.

        Returns:
            A list of :class:`ExtractedImage` dicts.
        """

    def image_count(self) -> int:
        """Total number of images in the document.

        Returns:
            Same as ``len(doc.images())``, without materialising the list.
        """

    def links(self) -> list[Link]:
        """All hyperlinks in the document.

        Returns:
            A list of :class:`Link` dicts.
        """

    def link_count(self) -> int:
        """Total number of hyperlinks in the document.

        Returns:
            Same as ``len(doc.links())``, without materialising the list.
        """

    def tables(self) -> list[Table]:
        """All reconstructed tables, including cross-page tables.

        Returns:
            A list of :class:`Table` dicts.
        """

    def table_count(self) -> int:
        """Total number of tables in the document.

        Returns:
            Same as ``len(doc.tables())``, without materialising the list.
        """

    def search(self, query: str) -> list[SearchHit]:
        """Search for a literal substring across the full document.

        The match is case-insensitive and substring-based.

        Args:
            query: The text to look for. An empty string returns no hits.

        Returns:
            A list of :class:`SearchHit` dicts.
        """

    def chunks_by_page(self) -> list[Chunk]:
        """One text chunk per page, suitable for RAG-style indexing.

        Returns:
            A list of :class:`Chunk` dicts.
        """

    def outline(self) -> list[OutlineEntry]:
        """Hierarchical outline (table of contents) of the document.

        Returns:
            A list of :class:`OutlineEntry` dicts.

        Raises:
            OlgaError: If the outline cannot be computed.
        """

    def to_json(self) -> DocumentJson:
        """Full document tree serialised into a JSON-compatible Python object.

        The result is a dict / list / scalar structure produced via
        :func:`json.loads`, so it is safe to re-serialise with
        :func:`json.dumps`. See :class:`~olgadoc.DocumentJson` for the
        exact schema and :data:`~olgadoc.JsonElement` for the discriminated
        union of element variants.

        Returns:
            A :class:`DocumentJson` payload carrying document metadata,
            per-page geometry, structural elements and any warnings.

        Raises:
            OlgaError: If serialisation fails.
        """

    def processability(self) -> Processability:
        """Compute a health report for the document.

        Call this before paying for downstream work to know whether
        extraction is reliable, degraded, or outright blocked.

        Returns:
            A :class:`Processability` instance describing blockers and
            degradations.
        """

    def __repr__(self) -> str: ...
