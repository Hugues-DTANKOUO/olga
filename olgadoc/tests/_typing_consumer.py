"""Static-typing smoke script — exercised only by ``mypy --strict``.

Not collected by pytest (leading underscore). The purpose is to make sure the
public stubs let real consumer code type-check without a single ``Any``. We
touch the surfaces most likely to break under stub drift: ``to_json()`` and
its discriminated ``JsonElement`` union, the ``FormatHint`` literal, plus the
richer ``TypedDict`` payloads (``Link``, ``Table``, ``SearchHit``, etc.).
"""

from __future__ import annotations

import olgadoc
from olgadoc import (
    DocumentJson,
    HealthLabel,
    JsonElement,
    JsonSource,
    JsonTableElement,
    Link,
    Processability,
    SearchHit,
    Table,
)

# ``assert_type`` landed in ``typing`` in Python 3.11; for older interpreters
# we fall back to the ``typing_extensions`` backport (semantically identical).
from typing_extensions import assert_type


def _consume_to_json(doc: olgadoc.Document) -> None:
    payload: DocumentJson = doc.to_json()

    # Source block is a TypedDict with exact keys — pick a couple.
    source: JsonSource = payload["source"]
    assert_type(source["format"], str)
    assert_type(source["encrypted"], bool)

    for element in payload["elements"]:
        _walk(element)

    # Warnings are optional on the top-level dict.
    for warning in payload.get("warnings", []):
        assert_type(warning["kind"], str)


def _walk(element: JsonElement) -> None:
    # Discriminated union: narrowing on ``type`` gives exact field access.
    if element["type"] == "table":
        table: JsonTableElement = element
        assert_type(table["rows"], int)
        assert_type(table["headers"], list[str])
        for row in table["data"]:
            assert_type(row, list[str])
    elif element["type"] == "heading":
        assert_type(element["level"], int)
        assert_type(element["text"], str)
    elif element["type"] == "image":
        # ``alt_text`` is ``Optional[str]`` — mypy must enforce the None case.
        alt = element["alt_text"]
        if alt is not None:
            assert_type(alt, str)

    for child in element.get("children", []):
        _walk(child)


def _consume_rich_payloads(doc: olgadoc.Document) -> None:
    for link in doc.links():
        link_typed: Link = link
        assert_type(link_typed["url"], str)
        assert_type(link_typed["bbox"]["x"], float)
    for table in doc.tables():
        table_typed: Table = table
        for cell in table_typed["cells"]:
            assert_type(cell["text"], str)
    for hit in doc.search("foo"):
        hit_typed: SearchHit = hit
        assert_type(hit_typed["snippet"], str)


def _consume_processability(doc: olgadoc.Document) -> None:
    report: Processability = doc.processability()
    health: HealthLabel = report.health
    # Literal discrimination should be respected.
    if health == "blocked":
        assert report.is_processable is False
