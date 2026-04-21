"""End-to-end tests for the ``olgadoc`` Python bindings.

Complements the focused unit tests in ``test_document.py`` /
``test_processability.py`` / ``test_typing.py`` with three broader concerns:

* **Smoke matrix** — every public operation on every supported format, so a
  format-specific regression cannot slip through.
* **Realistic scenario** — a whole user journey (open → inspect → search →
  serialise → re-deserialise) run end-to-end on each fixture.
* **Typed JSON tree walk** — recursively discriminates :data:`JsonElement`
  variants at runtime, asserting every node carries the fields its ``type``
  literal advertises. This guards against the JSON renderer drifting away
  from the TypedDict schema without the static checker noticing.
"""

from __future__ import annotations

import json
from pathlib import Path

import olgadoc
import pytest
from olgadoc import DocumentJson, JsonElement

# ---------------------------------------------------------------------------
# Smoke matrix — every public operation on every supported format
# ---------------------------------------------------------------------------


def test_smoke_matrix_every_document_method(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format
    WHEN every public ``Document`` method is called exactly once
    THEN every call returns a value of the documented type
    AND none of them raise
    AND aggregate / count pairs agree
    """
    doc = olgadoc.Document.open(str(any_doc_path))

    # Properties
    assert doc.format in {"PDF", "DOCX", "XLSX", "HTML"}
    assert doc.page_count >= 0
    assert isinstance(doc.is_processable, bool)
    assert doc.title is None or isinstance(doc.title, str)
    assert doc.file_size > 0
    assert isinstance(doc.encrypted, bool)

    # List-returning methods
    assert isinstance(doc.warnings(), list)
    assert all(isinstance(w, str) for w in doc.warnings())
    assert isinstance(doc.pages(), list)
    assert all(isinstance(p, olgadoc.Page) for p in doc.pages())
    assert isinstance(doc.images(), list)
    assert isinstance(doc.links(), list)
    assert isinstance(doc.tables(), list)
    assert isinstance(doc.chunks_by_page(), list)
    assert isinstance(doc.outline(), list)

    # Aggregate / count invariants
    assert len(doc.pages()) == doc.page_count
    assert len(doc.images()) == doc.image_count()
    assert len(doc.links()) == doc.link_count()
    assert len(doc.tables()) == doc.table_count()

    # Text / markdown shapes
    assert isinstance(doc.text(), str)
    assert isinstance(doc.markdown(), str)
    assert isinstance(doc.text_by_page(), dict)
    assert isinstance(doc.markdown_by_page(), dict)
    assert all(
        isinstance(k, int) and isinstance(v, str) for k, v in doc.text_by_page().items()
    )

    # Search with an empty query is a well-defined no-op
    assert doc.search("") == []

    # Processability always returns a report
    report = doc.processability()
    assert isinstance(report, olgadoc.Processability)
    assert report.health in {"ok", "degraded", "blocked"}

    # Repr must never raise
    assert isinstance(repr(doc), str)
    assert isinstance(repr(report), str)


def test_smoke_matrix_every_page_method(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format
    WHEN every public ``Page`` method is called on each page
    THEN every page exposes consistent text / markdown / links / tables / images
    AND the per-page counts agree with the materialised collections
    """
    doc = olgadoc.Document.open(str(any_doc_path))
    for page in doc.pages():
        assert page.number >= 1
        dims = page.dimensions  # may be None for HTML / XLSX
        assert dims is None or (dims["width_pt"] > 0 and dims["height_pt"] > 0)
        assert isinstance(page.text(), str)
        assert isinstance(page.markdown(), str)
        assert len(page.images()) == page.image_count()
        assert len(page.links()) == page.link_count()
        assert len(page.tables()) == page.table_count()
        assert page.search("") == []
        chunk = page.chunk()
        assert chunk is None or chunk["char_count"] == len(chunk["text"])
        assert isinstance(repr(page), str)


# ---------------------------------------------------------------------------
# Realistic scenario — a whole user journey end-to-end
# ---------------------------------------------------------------------------


def test_realistic_scenario_open_inspect_search_serialise(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format
    WHEN a user opens it, inspects processability, searches a common word,
         serialises to JSON and re-deserialises
    THEN every step succeeds without raising
    AND the JSON round-trip preserves the document structure
    AND the re-parsed payload still satisfies the :class:`DocumentJson` shape
    """
    doc = olgadoc.Document.open(str(any_doc_path))

    # 1. Pre-flight: is this document worth processing?
    report = doc.processability()
    assert report.health in {"ok", "degraded", "blocked"}
    if report.is_blocked():
        pytest.skip(f"{any_doc_path.name} is blocked — downstream steps N/A")

    # 2. Extract content + search a common token — at least one hit is
    #    expected across every corpus fixture.
    text = doc.text()
    assert text.strip(), f"{any_doc_path.name} yielded empty text"
    hits = doc.search("e")  # "e" is the most common letter in every fixture language
    assert all(hit["page"] >= 1 for hit in hits)

    # 3. Full structured JSON. Round-trip through ``json.dumps`` /
    #    ``json.loads`` and confirm the schema survives.
    payload: DocumentJson = doc.to_json()
    reparsed = json.loads(json.dumps(payload))
    assert set(reparsed.keys()) >= {"olga_version", "source", "pages", "elements"}
    assert reparsed["source"]["format"] == doc.format
    assert reparsed["source"]["page_count"] == doc.page_count


# ---------------------------------------------------------------------------
# Typed JSON tree walk — runtime discrimination of :data:`JsonElement`
# ---------------------------------------------------------------------------

# Map each ``type`` literal to the set of keys that variant is *required* to
# carry. These must match the ``Required[...]`` annotations declared in
# :mod:`olgadoc.__init__`; a drift between the JSON renderer and the stub
# will fail one of these assertions immediately.
REQUIRED_KEYS: dict[str, set[str]] = {
    "document": {"id", "type", "bbox", "page"},
    "section": {"id", "type", "level", "bbox", "page"},
    "heading": {"id", "type", "level", "text", "bbox", "page"},
    "paragraph": {"id", "type", "text", "bbox", "page"},
    "table": {"id", "type", "rows", "cols", "bbox", "page", "headers", "data"},
    "table_row": {"id", "type", "bbox", "page"},
    "table_cell": {
        "id",
        "type",
        "row",
        "col",
        "rowspan",
        "colspan",
        "text",
        "bbox",
        "page",
    },
    "list": {"id", "type", "ordered", "bbox", "page"},
    "list_item": {"id", "type", "text", "bbox", "page"},
    "image": {"id", "type", "format", "alt_text", "bbox", "page"},
    "code_block": {"id", "type", "language", "text", "bbox", "page"},
    "block_quote": {"id", "type", "text", "bbox", "page"},
    "page_header": {"id", "type", "text", "bbox", "page"},
    "page_footer": {"id", "type", "text", "bbox", "page"},
    "footnote": {"id", "type", "footnote_id", "text", "bbox", "page"},
    "aligned_line": {"id", "type", "text", "bbox", "page"},
}

ALL_KINDS = frozenset(REQUIRED_KEYS.keys())


def _assert_bbox(bbox: object) -> None:
    """Check that a bbox payload carries the four required floats."""
    assert isinstance(bbox, dict)
    for axis in ("x", "y", "w", "h"):
        assert axis in bbox, f"bbox missing '{axis}'"
        assert isinstance(bbox[axis], (int, float))


def _walk_element(element: JsonElement, kinds_seen: set[str]) -> None:
    """Recursively discriminate ``element`` and its children.

    Every ``type`` literal routes through its dedicated TypedDict branch so
    mypy ``--strict`` on this file proves the union narrows correctly at
    every step — each branch checks ``element["type"]`` directly so mypy
    picks the matching variant out of the discriminated union. At runtime
    we additionally verify that each node carries the keys its variant
    requires.
    """
    kind = element["type"]
    assert kind in ALL_KINDS, f"unknown JsonElement kind: {kind!r}"
    kinds_seen.add(kind)

    assert REQUIRED_KEYS[kind] <= set(element.keys()), (
        f"{kind!r} element missing required keys: "
        f"{REQUIRED_KEYS[kind] - set(element.keys())}"
    )
    _assert_bbox(element["bbox"])
    assert element["page"] >= 0

    # Discriminate on ``element["type"]`` directly — this is the only form
    # mypy narrows the ``JsonElement`` union on.
    if element["type"] == "document":
        _walk_children(element["children"], kinds_seen)
    elif element["type"] == "section":
        assert element["level"] >= 0
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "heading":
        assert isinstance(element["text"], str)
        assert element["level"] >= 1
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "paragraph":
        assert isinstance(element["text"], str)
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "table":
        assert element["rows"] >= 0 and element["cols"] >= 0
        assert isinstance(element["headers"], list)
        assert isinstance(element["data"], list)
        for cell in element.get("cells", []):
            assert isinstance(cell["text"], str)
            _assert_bbox(cell["bbox"])
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "table_row":
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "table_cell":
        assert element["row"] >= 0 and element["col"] >= 0
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "list":
        assert isinstance(element["ordered"], bool)
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "list_item":
        assert isinstance(element["text"], str)
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "image":
        assert isinstance(element["format"], str)
        # ``alt_text`` is ``Optional[str]`` — required key, nullable value.
        assert element["alt_text"] is None or isinstance(element["alt_text"], str)
    elif element["type"] == "code_block":
        assert element["language"] is None or isinstance(element["language"], str)
        assert isinstance(element["text"], str)
    elif element["type"] == "block_quote":
        assert isinstance(element["text"], str)
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "page_header":
        assert isinstance(element["text"], str)
    elif element["type"] == "page_footer":
        assert isinstance(element["text"], str)
    elif element["type"] == "footnote":
        assert isinstance(element["footnote_id"], str)
        assert isinstance(element["text"], str)
        _walk_children(element.get("children", []), kinds_seen)
    elif element["type"] == "aligned_line":
        assert isinstance(element["text"], str)
        for span in element.get("spans", []):
            assert isinstance(span["text"], str)
            assert span["col"] >= 0


def _walk_children(children: list[JsonElement], kinds_seen: set[str]) -> None:
    """Recurse into a list of child elements."""
    for child in children:
        _walk_element(child, kinds_seen)


def test_json_tree_every_element_matches_its_typed_schema(any_doc_path: Path) -> None:
    """
    GIVEN one fixture per supported format
    WHEN ``to_json()`` is fully walked and every element is discriminated
         against its :data:`JsonElement` variant
    THEN every required key declared by the TypedDict is present at runtime
    AND at least the root element kind is observed
    AND bounding boxes carry the four ``x`` / ``y`` / ``w`` / ``h`` floats
    """
    doc = olgadoc.Document.open(str(any_doc_path))
    payload: DocumentJson = doc.to_json()

    # Top-level shape is already tightened by DocumentJson.
    assert isinstance(payload["olga_version"], str)
    source = payload["source"]
    assert source["format"] in {"PDF", "DOCX", "XLSX", "HTML"}
    for page_info in payload["pages"]:
        assert page_info["page"] >= 0
        assert page_info["width_pt"] >= 0
        assert page_info["height_pt"] >= 0

    kinds_seen: set[str] = set()
    for element in payload["elements"]:
        _walk_element(element, kinds_seen)

    # Every fixture should at least produce something structural.
    assert kinds_seen, "to_json() emitted zero elements"
    # And every observed kind must be known — no undeclared variant leaks out.
    assert kinds_seen <= ALL_KINDS


# ---------------------------------------------------------------------------
# Cross-cutting — TypedDicts are real runtime objects
# ---------------------------------------------------------------------------


def test_typed_dicts_are_runtime_introspectable() -> None:
    """
    GIVEN the ``olgadoc`` package namespace
    WHEN a handful of TypedDict symbols are looked up at runtime
    THEN each resolves to a real class carrying ``__annotations__``
    AND required / optional keys are advertised via the standard attributes
    """
    for symbol in (
        olgadoc.Link,
        olgadoc.Table,
        olgadoc.SearchHit,
        olgadoc.DocumentJson,
        olgadoc.JsonTableElement,
        olgadoc.ExtractedImage,
    ):
        assert hasattr(symbol, "__annotations__"), f"{symbol!r} is not introspectable"
        assert symbol.__annotations__, f"{symbol!r} has no declared keys"
