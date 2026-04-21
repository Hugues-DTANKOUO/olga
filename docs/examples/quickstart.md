# Quickstart example

A "hello world" that opens one or more documents and prints the
detected format, page count, title and a 60-character preview of every
page's text.

## Usage

```bash
python examples/quickstart.py PATH [PATH ...]
```

`PATH` can be any supported document (PDF, DOCX, XLSX, HTML). The exit
code is the worst status across all inputs.

## Source

```python
--8<-- "olgadoc/examples/quickstart.py"
```
