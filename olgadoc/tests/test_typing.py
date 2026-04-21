"""Sanity checks for the shipped type stubs.

Ensures the ``.pyi`` stub file and PEP 561 marker travel with the installed
package, that the stub parses as valid Python, and that every symbol it
declares exists at runtime. Gives a cheap canary for stub / implementation
drift without pulling a type checker into the unit-test loop.
"""

from __future__ import annotations

import ast
from pathlib import Path

import olgadoc

PACKAGE_DIR = Path(olgadoc.__file__).parent
STUB_FILE = PACKAGE_DIR / "olgadoc.pyi"
MARKER = PACKAGE_DIR / "py.typed"


def test_py_typed_marker_is_shipped() -> None:
    """
    GIVEN an installed ``olgadoc`` package
    WHEN the package directory is inspected
    THEN a PEP 561 ``py.typed`` marker is present
    """
    assert MARKER.is_file(), f"missing PEP 561 marker at {MARKER}"


def test_pyi_stub_is_shipped_and_parses() -> None:
    """
    GIVEN an installed ``olgadoc`` package
    WHEN the ``olgadoc.pyi`` stub file is loaded
    THEN the file exists at the expected location
    AND its contents are syntactically valid Python
    AND the parsed module is non-empty
    """
    assert STUB_FILE.is_file(), f"missing stub at {STUB_FILE}"
    tree = ast.parse(STUB_FILE.read_text(), filename=str(STUB_FILE))
    assert tree.body


def _top_level_names(tree: ast.Module) -> set[str]:
    """Collect the top-level class and variable names declared in a stub."""
    out: set[str] = set()
    for node in tree.body:
        if isinstance(node, ast.ClassDef):
            out.add(node.name)
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            out.add(node.target.id)
        elif isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name):
                    out.add(target.id)
    return out


def test_stub_declares_exported_classes_and_version() -> None:
    """
    GIVEN the parsed ``olgadoc.pyi`` stub
    WHEN its top-level names are collected
    THEN ``Document``, ``Page``, ``Processability``, ``OlgaError`` are declared
    AND the ``__version__`` attribute is declared
    """
    tree = ast.parse(STUB_FILE.read_text())
    names = _top_level_names(tree)
    for required in ("Document", "Page", "Processability", "OlgaError", "__version__"):
        assert required in names, f"stub missing declaration for '{required}'"


def test_stub_class_methods_exist_on_runtime_objects() -> None:
    """
    GIVEN the runtime classes exported from ``olgadoc``
    WHEN a handful of methods and properties declared in the stub are probed
    THEN ``Document.open`` and ``Document.open_bytes`` are callable
    AND every declared ``Processability`` getter exists on the class
    AND every declared ``Page`` getter exists on the class
    """
    assert callable(olgadoc.Document.open)
    assert callable(olgadoc.Document.open_bytes)
    for name in (
        "health",
        "is_processable",
        "pages_total",
        "pages_with_content",
        "warning_count",
        "blockers",
        "degradations",
    ):
        assert hasattr(olgadoc.Processability, name), (
            f"missing {name} on Processability"
        )
    for name in ("number", "dimensions"):
        assert hasattr(olgadoc.Page, name), f"missing {name} on Page"
