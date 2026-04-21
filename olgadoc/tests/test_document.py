"""End-to-end tests for :class:`olgadoc.Document`.

Loads real corpus fixtures and exercises the full public surface of the
Python bindings — opening, page iteration, text and markdown extraction,
images, links, tables, search, chunks, outline, JSON round-trip and error
paths.
"""

from __future__ import annotations

import json
from pathlib import Path

import olgadoc
import pytest

# ---------------------------------------------------------------------------
# Opening
# ---------------------------------------------------------------------------


def test_open_pdf_reports_format_and_page_count(pdf_path: Path) -> None:
    """
    GIVEN a structured PDF fixture on disk
    WHEN the document is opened with ``Document.open``
    THEN it reports format ``"PDF"`` with a positive page_count
    AND it is processable, unencrypted, and has a non-zero file_size
    """
    doc = olgadoc.Document.open(str(pdf_path))
    assert doc.format == "PDF"
    assert doc.page_count >= 1
    assert doc.is_processable is True
    assert doc.encrypted is False
    assert doc.file_size > 0


def test_open_every_supported_format(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format (PDF, DOCX, XLSX, HTML)
    WHEN each is opened with ``Document.open``
    THEN the resulting document advertises a known format label
    AND it has at least one page
    """
    doc = olgadoc.Document.open(str(any_doc_path))
    assert doc.format in {"PDF", "DOCX", "XLSX", "HTML"}
    assert doc.page_count >= 1


def test_open_bytes_round_trip(pdf_path: Path) -> None:
    """
    GIVEN the bytes of a PDF fixture
    WHEN the document is opened with ``open_bytes`` both with and without a hint
    THEN both calls yield equivalent documents
    AND magic-byte inference matches the explicit ``"pdf"`` hint
    """
    data = pdf_path.read_bytes()
    doc_auto = olgadoc.Document.open_bytes(data)
    assert doc_auto.format == "PDF"
    doc_hint = olgadoc.Document.open_bytes(data, format="pdf")
    assert doc_hint.format == "PDF"
    assert doc_auto.page_count == doc_hint.page_count


def test_open_bytes_rejects_unknown_format_hint(pdf_path: Path) -> None:
    """
    GIVEN valid PDF bytes
    WHEN ``open_bytes`` is called with an unknown format hint
    THEN ``OlgaError`` is raised
    AND the error message mentions the unknown hint
    """
    data = pdf_path.read_bytes()
    # Deliberately bypass the ``FormatHint`` :data:`Literal` narrowing to
    # exercise the runtime validation path in the Rust layer — a strictly
    # typed caller is already protected at compile time.
    bogus_hint: str = "not-a-real-format"
    with pytest.raises(olgadoc.OlgaError) as excinfo:
        olgadoc.Document.open_bytes(data, format=bogus_hint)  # type: ignore[arg-type]
    assert "unknown format hint" in str(excinfo.value).lower()


def test_open_missing_file_raises_olga_error(tmp_path: Path) -> None:
    """
    GIVEN a path to a file that does not exist
    WHEN ``Document.open`` is called with that path
    THEN ``OlgaError`` is raised
    """
    missing = tmp_path / "nope.pdf"
    with pytest.raises(olgadoc.OlgaError):
        olgadoc.Document.open(str(missing))


# ---------------------------------------------------------------------------
# Text & Markdown
# ---------------------------------------------------------------------------


def test_text_is_non_empty_for_pdf(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``text()`` is called
    THEN the result is a string
    AND it contains at least one non-whitespace character
    """
    doc = olgadoc.Document.open(str(pdf_path))
    text = doc.text()
    assert isinstance(text, str)
    assert text.strip(), "structured_report.pdf must yield non-empty text"


def test_markdown_is_non_empty_for_pdf(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``markdown()`` is called
    THEN the result is a string
    AND it contains at least one non-whitespace character
    """
    doc = olgadoc.Document.open(str(pdf_path))
    md = doc.markdown()
    assert isinstance(md, str)
    assert md.strip()


def test_text_by_page_is_keyed_by_page_number(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``text_by_page()`` is called
    THEN the result is a non-empty dict
    AND every key is a positive integer
    AND every value is a string
    """
    doc = olgadoc.Document.open(str(pdf_path))
    by_page = doc.text_by_page()
    assert isinstance(by_page, dict)
    assert by_page, "text_by_page must not be empty"
    for key, val in by_page.items():
        assert isinstance(key, int) and key >= 1
        assert isinstance(val, str)


def test_markdown_by_page_matches_page_count(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``markdown_by_page()`` is called
    THEN every key is a valid 1-based page number bounded by ``page_count``
    """
    doc = olgadoc.Document.open(str(pdf_path))
    by_page = doc.markdown_by_page()
    assert set(by_page.keys()) <= set(range(1, doc.page_count + 1))


# ---------------------------------------------------------------------------
# Pages
# ---------------------------------------------------------------------------


def test_pages_returns_one_page_per_page_count(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``pages()`` is called
    THEN the returned list has exactly ``page_count`` entries
    AND each entry is a :class:`Page` numbered consecutively from 1
    """
    doc = olgadoc.Document.open(str(pdf_path))
    pages = doc.pages()
    assert len(pages) == doc.page_count
    for idx, page in enumerate(pages, start=1):
        assert isinstance(page, olgadoc.Page)
        assert page.number == idx


def test_page_lookup_by_number(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``page(1)`` and ``page(page_count + 1)`` are called
    THEN the in-range lookup returns a :class:`Page` with ``number == 1``
    AND the out-of-range lookup returns ``None`` instead of raising
    """
    doc = olgadoc.Document.open(str(pdf_path))
    first = doc.page(1)
    assert first is not None
    assert first.number == 1
    assert doc.page(doc.page_count + 1) is None


def test_page_text_matches_text_by_page(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN every ``page.text()`` is compared to ``text_by_page()``
    THEN the per-page text matches the aggregate by page number
    """
    doc = olgadoc.Document.open(str(pdf_path))
    expected = doc.text_by_page()
    for page in doc.pages():
        assert page.text() == expected.get(page.number, "")


def test_page_dimensions_present_for_pdf(pdf_path: Path) -> None:
    """
    GIVEN the first page of a PDF document
    WHEN ``dimensions`` is read
    THEN the result is a dict (PDF carries page geometry)
    AND ``width_pt`` / ``height_pt`` are positive
    AND ``rotation`` is an integer
    """
    doc = olgadoc.Document.open(str(pdf_path))
    page = doc.page(1)
    assert page is not None, "structured_report.pdf must expose page 1"
    dims = page.dimensions
    assert dims is not None, "PDF pages must report physical dimensions"
    assert dims["width_pt"] > 0
    assert dims["height_pt"] > 0
    assert isinstance(dims["rotation"], int)


# ---------------------------------------------------------------------------
# Search
# ---------------------------------------------------------------------------


def test_search_returns_hits_with_expected_shape(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``search("the")`` is called
    THEN at least one hit is returned
    AND each hit carries page, line, col_start, match and snippet
    AND every snippet contains its match
    """
    doc = olgadoc.Document.open(str(pdf_path))
    hits = doc.search("the")
    assert isinstance(hits, list)
    assert hits, "'the' should match at least once in the structured report"
    for hit in hits:
        assert set(hit.keys()) >= {"page", "line", "col_start", "match", "snippet"}
        assert hit["match"].lower() in hit["snippet"].lower()
        assert hit["page"] >= 1


def test_search_empty_query_returns_empty_list(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``search("")`` is called
    THEN the result is an empty list
    """
    doc = olgadoc.Document.open(str(pdf_path))
    assert doc.search("") == []


def test_page_search_restricts_to_page(pdf_path: Path) -> None:
    """
    GIVEN the first page of an opened PDF document
    WHEN ``page.search("the")`` is called
    THEN every returned hit has ``page == 1``
    """
    doc = olgadoc.Document.open(str(pdf_path))
    page1 = doc.page(1)
    assert page1 is not None
    for hit in page1.search("the"):
        assert hit["page"] == 1


# ---------------------------------------------------------------------------
# Images / Links / Tables
# ---------------------------------------------------------------------------


def test_images_aggregate_matches_image_count(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format
    WHEN ``images()`` and ``image_count()`` are compared
    THEN ``len(images())`` equals ``image_count()``
    """
    doc = olgadoc.Document.open(str(any_doc_path))
    assert len(doc.images()) == doc.image_count()


def test_links_aggregate_matches_link_count(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format
    WHEN ``links()`` and ``link_count()`` are compared
    THEN ``len(links())`` equals ``link_count()``
    """
    doc = olgadoc.Document.open(str(any_doc_path))
    assert len(doc.links()) == doc.link_count()


def test_tables_aggregate_matches_table_count(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format
    WHEN ``tables()`` and ``table_count()`` are compared
    THEN ``len(tables())`` equals ``table_count()``
    """
    doc = olgadoc.Document.open(str(any_doc_path))
    assert len(doc.tables()) == doc.table_count()


def test_image_dict_shape(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format
    WHEN ``images()`` is iterated
    THEN every image carries page, format, alt_text, bbox, data and size
    AND ``data`` is bytes
    AND ``size`` matches ``len(data)``
    AND ``page`` is 1-based (>= 1)
    """
    doc = olgadoc.Document.open(str(any_doc_path))
    for img in doc.images():
        assert {"page", "format", "alt_text", "bbox", "data", "size"} <= set(img.keys())
        assert isinstance(img["data"], bytes)
        assert img["size"] == len(img["data"])
        assert img["page"] >= 1


# ---------------------------------------------------------------------------
# Chunks / Outline / JSON
# ---------------------------------------------------------------------------


def test_chunks_by_page_carries_char_count(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``chunks_by_page()`` is called
    THEN at least one chunk is returned
    AND each chunk carries page, text and char_count
    AND ``char_count`` equals ``len(text)``
    """
    doc = olgadoc.Document.open(str(pdf_path))
    chunks = doc.chunks_by_page()
    assert chunks
    for chunk in chunks:
        assert {"page", "text", "char_count"} <= set(chunk.keys())
        assert chunk["char_count"] == len(chunk["text"])


def test_outline_entries_carry_level_and_page(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``outline()`` is iterated
    THEN every entry carries level, text, page and bbox
    AND ``level`` is greater than or equal to 1
    """
    doc = olgadoc.Document.open(str(pdf_path))
    for entry in doc.outline():
        assert {"level", "text", "page", "bbox"} <= set(entry.keys())
        assert entry["level"] >= 1


def test_to_json_is_serializable(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``to_json()`` is called
    THEN the result is round-trippable through :func:`json.dumps`
    """
    doc = olgadoc.Document.open(str(pdf_path))
    payload = doc.to_json()
    assert json.dumps(payload) is not None


# ---------------------------------------------------------------------------
# Warnings / repr
# ---------------------------------------------------------------------------


def test_warnings_is_list_of_strings(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format
    WHEN ``warnings()`` is called
    THEN the result is a list
    AND every element is a string
    """
    doc = olgadoc.Document.open(str(any_doc_path))
    ws = doc.warnings()
    assert isinstance(ws, list)
    assert all(isinstance(w, str) for w in ws)


def test_repr_mentions_format(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``repr(doc)`` is called
    THEN the string starts with ``"Document("``
    AND it mentions ``format='PDF'``
    """
    doc = olgadoc.Document.open(str(pdf_path))
    r = repr(doc)
    assert r.startswith("Document(")
    assert "format='PDF'" in r
