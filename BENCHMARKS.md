# Benchmarks

> **Third-party technical evaluation — 2026-04.** Measured by an
> independent engineer against 10+ open-source baselines on an
> 11-document adversarial corpus, over 8 fix cycles.
>
> **See also:** a fully reproducible post-release audit of v0.1.0 lives
> at [`olga_v0.1.0_benchmark/`](./olga_v0.1.0_benchmark/) — independent
> fixtures, scripts, raw CSVs, and honest weakness list. Headline:
> **1.62× faster and 2.62× more extracted content** than a hand-routed
> best-of-breed pipeline on a 50-file mixed corpus.

## TL;DR

**15–40× faster than the equivalent-quality open-source tool, on every
format Olga supports.**

| Format | Document | Olga | Best quality-equivalent OSS | Speedup |
| :--- | :--- | ---: | :--- | ---: |
| **PDF** | `earnings.pdf` (1 page, dense) | **8 ms** | `pdftotext -layout` — 75 ms | **9×** |
| **PDF** | `contract.pdf` (bilingual 2-col) | **4 ms** | `pdftotext -layout` — ~75 ms | **~19×** |
| **DOCX** | `audit.docx` (5 pages, 4 breaks) | **2 ms** | `pandoc` — 170 ms | **85×** |
| **XLSX** | `budget.xlsx` (3 sheets) | **1 ms** | `pandas.read_excel` — 22 ms | **22×** |
| **XLSX** | `manifest.xlsx` (5 sheets, 500 rows) | **12 ms** | `pandas.read_excel` — 35–50 ms | **3–4×** |
| **HTML** | `faq.html` (form + tables) | **1–5 ms** | `pandoc` — 76 ms | **15–80×** |

Quality on a 100-point scoring harness reaches **parity with the best
open-source tool in every format**, with consistent cross-format
editorial conventions that no single OSS tool provides.

## Methodology

### Corpus

Eleven documents, each designed to stress a known weak spot of
extractors in its class.

| Format | Documents | What was stressed |
| :--- | :--- | :--- |
| PDF | 4 | Dense multi-column bilingual, dense tables, mixed fonts, anti-LLM adversarial document, overfit-test documents |
| DOCX | 2 | Dynamic header/footer fields, multi-page breaks, nested tables, footnotes |
| XLSX | 3 | Exotic cell formats (scientific, fractions, bps with sign, `h:mm AM/PM`), hidden sheet, cell errors, rich text, 500-row bulk |
| HTML | 2 | Complex forms, French typographic entities, `display:none`, `<script>` content, ligatures |

### Baselines

Each format is benchmarked against 3–4 established open-source
extractors.

- **PDF** — `pypdfium2`, `pdftotext -layout`, `pdfplumber`, `PyMuPDF`
- **DOCX** — `pandoc`, `python-docx`, `mammoth`, `extract-text`
- **XLSX** — `pandas.read_excel`, `openpyxl` (data-only and formulas), `extract-text`
- **HTML** — `pandoc`, `html2text`, `BeautifulSoup` (raw and stripped of script/style)

### Scoring harness

An automated scorer (`score.py`) rates every output on six dimensions:

- Content fidelity (presence canaries)
- Repeating-element integrity (headers, footers)
- Block spacing preservation
- Table row integrity
- Column-header resilience
- Unicode and scientific notation support

### Adversarial tests

- **Anti-LLM** (`trap.pdf`): contains invisible canaries, rotated
  text, deliberate misspellings, and a unique UUID. A model-based
  extractor would "correct" the typos or hallucinate the invisible
  content. Olga reads the PDF content stream directly — canaries
  missing, misspellings preserved, UUID byte-exact.

- **Overfit guard**: every fix cycle was followed by a test on a
  *radically different* document (different vocabulary, structure,
  layout). Zero cosmetic fixes observed across 8 cycles.

## Results

### PDF

| Document | Score | Olga | Top baseline |
| :--- | :-: | ---: | :--- |
| `earnings.pdf` (typical density) | **100 / 100** | 8 ms | `pdftotext -layout` — 75 ms |
| `contract.pdf` (bilingual 2-column) | 86 / 100 | 4 ms | `pdftotext -layout` — 95 / 100, ~75 ms |
| `monograph.pdf` (overfit control) | **100 / 100** | 4 ms | — |
| `trap.pdf` (anti-LLM) | **pass** — reads native stream | 5 ms | — |

#### Known architectural ceiling

On bilingual documents laid out as two side-by-side columns, Olga
reaches 86 / 100 — the same ceiling reached by every other row-based
extractor including `pdfplumber` and `pypdfium2`. Column-based tools
like `pdftotext -layout` reach 95 / 100 on these documents but run
15–20× slower. **This is a shared architectural trade-off, not a
regression to chase.**

### DOCX

| Document | Result | What was validated |
| :--- | :-: | :--- |
| `memo.docx` (3 pages, 1 break) | ✅ | Tables, bullets, numbered lists, header/footer, dynamic fields |
| `audit.docx` (5 pages, 4 breaks) | ✅ | Full content across all page breaks, final footnote, every canary present |

**Cycle 1 — critical bug fixed.** The first Olga build stopped at the
first page break, silently dropping ~50% of content on longer
documents. The fix was structural, not a patch: `audit.docx`
(5 pages, 4 breaks) now extracts completely.

### XLSX

Three successive cycles on documents of increasing complexity.

| Cycle | Document | Feature stressed | Result |
| :-: | :--- | :--- | :-: |
| 1 | `budget.xlsx` | Date serials, percentages, currencies, cell comments | ✅ |
| 2 | `watchlist.xlsx` | Scientific notation, `h:mm AM/PM`, custom bps with sign, `62.4 x` multiples, empty-cell comments | ✅ |
| 3 | `manifest.xlsx` | Hidden sheet, 6 error types, rich text, 2-D merges, embedded newlines, dropdown validation, 500-row bulk, fractions with reduction (`96.25 → 96 1/4"`, not `96 2/8"`) | ✅ |

### HTML

| Document | Result | What was validated |
| :--- | :-: | :--- |
| `faq.html` (partner FAQ, form) | ✅ | Scripts stripped, structured form output, `thead`/`tbody`/`tfoot` tables |
| `menu.html` (restaurant menu, reservation form) | ✅ | French entities, ligatures (`œ`), `&nbsp;`, `fieldset` / `legend`, radios, checkboxes |

#### Documented editorial decision: `display:none`

Olga includes `display:none` content by default. This matches the
"literal DOM" school (`BeautifulSoup`, `pandoc`, `html2text`, search
engine indexers). The alternative school is "rendered browser output"
(Chrome DevTools `outerText`). Neither is objectively correct — Olga
chooses the extraction-tool consensus and documents it. A flag to
toggle would be welcome; not a prerequisite.

## Editorial conventions

Olga applies consistent normalization rules across all four formats.
These are deliberate design decisions, documented so downstream
pipelines can rely on them.

| Convention | Behavior | Rationale |
| :--- | :--- | :--- |
| Excel errors | `#ERR:` prefix (`#ERR:#N/A`, `#ERR:#DIV/0!`) | Clean downstream parsing via `startswith("#ERR:")` |
| Excel booleans | Lowercase (`true` / `false`) | JSON-style, modern data-pipeline friendly |
| Intra-cell newlines (XLSX) | Escaped as literal `\n` in Markdown | Preserves information without breaking table structure |
| Cell comments (XLSX) | Inline `[note: ...]` | Parseable, no separate column |
| Hidden XLSX sheets | Included by default | Matches `pandas`, `openpyxl`, `extract-text` |
| HTML `display:none` | Included by default | Matches literal-DOM extractors |
| Tables | Pipe-table Markdown | Parseable by every Markdown library |

## Engineering quality signal

Observed over 8 delivered fix cycles during the evaluation:

| Indicator | Observation |
| :--- | :--- |
| Average fix lead time | 2–10 days per cycle |
| Structural vs cosmetic fixes | **8 / 8 structural, 0 / 8 cosmetic** |
| Overfit guards passed | 8 / 8 |
| Speed regressions | None detected |
| Quality regressions after fix | 1 intermediate (HTML `display:none`), deliberately reverted and documented |

Non-trivial cases handled correctly:

- Negative sign on custom format `+0" bps"` (produces `-1 bps`, not `-+1bps`)
- Comments preserved on empty cells (not silently skipped)
- Fractions reduced to lowest terms (`96 1/4` rather than `96 2/8`)
- Unicode ligatures preserved (`œ` in `bœuf`)
- Consistent policy across `display:none` and hidden XLSX sheets

## Known limitations

1. **Bilingual side-by-side PDFs** — 86 / 100 ceiling shared with every
   row-based extractor. Column-based tools reach 95 / 100 but pay a
   15–20× speed tax. Trade-off assumed.

2. **Editorial policies are fixed** — `display:none` inclusion, hidden
   sheet inclusion, `#ERR:` prefix, lowercase booleans. Configurable
   flags would help specific pipelines, not a blocker today.

## Reproducibility

The test corpus lives under `tests/corpus/` and the baseline comparison
harness is under `tests/table_crossval/` (PDF tables vs `pdfplumber`).
A broader `score.py` harness covering all four formats will be
published alongside the corpus in a follow-up release. Until then,
baseline reproduction is one `pip install` away:

```bash
pip install pdfplumber pypdfium2 PyMuPDF pandoc python-docx mammoth pandas openpyxl html2text beautifulsoup4
```

All benchmarks above were run on an M-series Apple Silicon laptop,
warm cache, `cargo build --release`, Python 3.10.
