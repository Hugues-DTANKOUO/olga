"""Shared fixtures for the ``olgadoc`` test suite.

Provides ready-made paths to the corpus documents used throughout the
tests — one fixture per format plus a parametrised fixture that iterates
over every supported format.
"""

from __future__ import annotations

from pathlib import Path
from typing import cast

import pytest

# olgadoc/tests/conftest.py -> olgadoc/ -> olga/
OLGA_ROOT = Path(__file__).resolve().parent.parent.parent
CORPUS = OLGA_ROOT / "tests" / "corpus"


@pytest.fixture(scope="session")
def corpus_dir() -> Path:
    """Return the absolute path of the shared corpus directory.

    Fails the whole session if the directory is missing so tests never
    silently pass against a phantom location.
    """
    if not CORPUS.is_dir():
        pytest.fail(f"corpus directory missing: {CORPUS}")
    return CORPUS


@pytest.fixture
def pdf_path(corpus_dir: Path) -> Path:
    """Path to the canonical PDF fixture."""
    return corpus_dir / "pdf" / "structured_report.pdf"


@pytest.fixture
def docx_path(corpus_dir: Path) -> Path:
    """Path to the canonical DOCX fixture."""
    return corpus_dir / "docx" / "project_status.docx"


@pytest.fixture
def xlsx_path(corpus_dir: Path) -> Path:
    """Path to the canonical XLSX fixture."""
    return corpus_dir / "xlsx" / "employee_directory.xlsx"


@pytest.fixture
def html_path(corpus_dir: Path) -> Path:
    """Path to the canonical HTML fixture."""
    return corpus_dir / "html" / "complex_report.html"


@pytest.fixture(
    params=[
        ("pdf", "structured_report.pdf"),
        ("docx", "project_status.docx"),
        ("xlsx", "employee_directory.xlsx"),
        ("html", "complex_report.html"),
    ],
    ids=["pdf", "docx", "xlsx", "html"],
)
def any_doc_path(corpus_dir: Path, request: pytest.FixtureRequest) -> Path:
    """Yield one fixture per supported document format."""
    # ``pytest.FixtureRequest.param`` is typed as ``Any`` by pytest — narrow
    # it explicitly so the downstream path construction stays strictly typed.
    fmt, name = cast("tuple[str, str]", request.param)
    return corpus_dir / fmt / name
