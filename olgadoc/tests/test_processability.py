"""Tests for :class:`olgadoc.Processability` — the document health report.

Verifies the shape, predicates and dict payloads of the processability
report across every supported document format.
"""

from __future__ import annotations

from pathlib import Path

import olgadoc

VALID_HEALTH = {"ok", "degraded", "blocked"}
KNOWN_KINDS = {
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
}


def test_processability_shape(pdf_path: Path) -> None:
    """
    GIVEN an opened PDF document
    WHEN ``processability()`` is called
    THEN the result is a :class:`Processability` instance
    AND ``health`` is a known label
    AND ``pages_total`` matches the document's ``page_count``
    AND ``pages_with_content`` is bounded by ``pages_total``
    AND ``warning_count`` is non-negative
    AND ``blockers`` and ``degradations`` are lists
    """
    doc = olgadoc.Document.open(str(pdf_path))
    report = doc.processability()
    assert isinstance(report, olgadoc.Processability)
    assert report.health in VALID_HEALTH
    assert isinstance(report.is_processable, bool)
    assert report.pages_total == doc.page_count
    assert 0 <= report.pages_with_content <= report.pages_total
    assert report.warning_count >= 0
    assert isinstance(report.blockers, list)
    assert isinstance(report.degradations, list)


def test_health_predicates_are_mutually_exclusive(pdf_path: Path) -> None:
    """
    GIVEN a processability report for an opened PDF document
    WHEN ``is_ok()``, ``is_degraded()`` and ``is_blocked()`` are evaluated
    THEN exactly one predicate returns ``True`` for any given verdict
    """
    doc = olgadoc.Document.open(str(pdf_path))
    report = doc.processability()
    votes = [report.is_ok(), report.is_degraded(), report.is_blocked()]
    assert sum(votes) == 1


def test_processability_every_format_is_processable(any_doc_path: Path) -> None:
    """
    GIVEN one clean fixture per supported format
    WHEN ``processability()`` is called
    THEN ``is_processable`` is ``True``
    AND ``blockers`` is empty
    """
    doc = olgadoc.Document.open(str(any_doc_path))
    r = doc.processability()
    assert r.is_processable is True
    assert r.blockers == []


def test_issue_dicts_use_known_kinds(pdf_path: Path) -> None:
    """
    GIVEN a processability report for an opened PDF document
    WHEN every issue in ``blockers`` and ``degradations`` is inspected
    THEN each carries a ``kind`` field
    AND each ``kind`` belongs to the known set of variants
    """
    doc = olgadoc.Document.open(str(pdf_path))
    r = doc.processability()
    for issue in r.blockers + r.degradations:
        assert "kind" in issue
        assert issue["kind"] in KNOWN_KINDS


def test_repr_contains_health_label(pdf_path: Path) -> None:
    """
    GIVEN a processability report for an opened PDF document
    WHEN ``repr(report)`` is called
    THEN the string starts with ``"Processability("``
    AND it embeds the report's ``health`` label
    """
    doc = olgadoc.Document.open(str(pdf_path))
    r = doc.processability()
    rep = repr(r)
    assert rep.startswith("Processability(")
    assert f"health='{r.health}'" in rep
