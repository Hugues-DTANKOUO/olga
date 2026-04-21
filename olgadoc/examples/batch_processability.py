"""Batch health-check a directory tree of documents.

Usage:
    python examples/batch_processability.py ROOT [--json]

Walks ``ROOT`` recursively, opens every supported document, and prints a
one-line health verdict per file — ``ok``, ``degraded`` (with the list of
degradation kinds) or ``blocked`` (with the blocker kinds). With
``--json``, emits one JSON object per line instead, suitable for piping
into ``jq`` or a downstream ingestion pipeline.

The exit code is non-zero when at least one document is blocked, so this
doubles as a pre-flight gate for batch processing jobs.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import sys
from pathlib import Path
from typing import Iterator

import olgadoc

SUPPORTED_SUFFIXES = {".pdf", ".docx", ".xlsx", ".html", ".htm"}


@dataclasses.dataclass(frozen=True)
class Verdict:
    """One processability verdict for a single file."""

    path: str
    health: str  # "ok" | "degraded" | "blocked" | "error"
    pages_total: int = 0
    pages_with_content: int = 0
    warning_count: int = 0
    blockers: tuple[str, ...] = ()
    degradations: tuple[str, ...] = ()
    error: str | None = None

    def to_dict(self) -> dict[str, object]:
        """Render as a plain JSON-serialisable dict."""
        return dataclasses.asdict(self)


def _iter_documents(root: Path) -> Iterator[Path]:
    """Yield every file under ``root`` whose suffix we can open."""
    for entry in sorted(root.rglob("*")):
        if entry.is_file() and entry.suffix.lower() in SUPPORTED_SUFFIXES:
            yield entry


def _check(path: Path) -> Verdict:
    """Run :meth:`Document.processability` and pack the verdict."""
    try:
        doc = olgadoc.Document.open(str(path))
    except olgadoc.OlgaError as exc:
        return Verdict(path=str(path), health="error", error=str(exc))
    report = doc.processability()
    return Verdict(
        path=str(path),
        health=report.health,
        pages_total=report.pages_total,
        pages_with_content=report.pages_with_content,
        warning_count=report.warning_count,
        blockers=tuple(issue["kind"] for issue in report.blockers),
        degradations=tuple(issue["kind"] for issue in report.degradations),
    )


def _format_line(verdict: Verdict) -> str:
    """Render a verdict as a compact aligned line."""
    health = verdict.health.upper()
    if health == "OK":
        return f"{health:<9} {verdict.path}"
    if health == "ERROR":
        return f"{health:<9} {verdict.path} ({verdict.error})"
    issues = verdict.blockers if health == "BLOCKED" else verdict.degradations
    issues_txt = ", ".join(issues) or "-"
    return f"{health:<9} {verdict.path} [{issues_txt}]"


def main(argv: list[str]) -> int:
    """CLI entry point. Returns a process exit code."""
    parser = argparse.ArgumentParser(
        description="Batch-check document processability.",
    )
    parser.add_argument("root", type=Path, help="directory to walk recursively")
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit one JSON object per line instead of pretty text",
    )
    args = parser.parse_args(argv[1:])

    if not args.root.is_dir():
        print(f"{args.root}: not a directory", file=sys.stderr)
        return 2

    worst = "ok"
    ranks = {"ok": 0, "degraded": 1, "error": 2, "blocked": 3}
    for path in _iter_documents(args.root):
        verdict = _check(path)
        if args.json:
            print(json.dumps(verdict.to_dict()))
        else:
            print(_format_line(verdict))
        if ranks.get(verdict.health, 0) > ranks.get(worst, 0):
            worst = verdict.health

    # Exit non-zero if any document blocked processing — useful in CI gates.
    return 1 if worst in {"blocked", "error"} else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
