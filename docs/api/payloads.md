# Payloads

Every method that returns a dict returns a real `TypedDict` — these are
runtime classes, introspectable with `__annotations__`, and fully typed
for `mypy --strict` consumers.

## Geometry

::: olgadoc.BoundingBox
    options:
      show_root_heading: true

::: olgadoc.PageDimensions
    options:
      show_root_heading: true

## Extracted content

::: olgadoc.Link
    options:
      show_root_heading: true

::: olgadoc.Table
    options:
      show_root_heading: true

::: olgadoc.TableCell
    options:
      show_root_heading: true

::: olgadoc.SearchHit
    options:
      show_root_heading: true

::: olgadoc.Chunk
    options:
      show_root_heading: true

::: olgadoc.OutlineEntry
    options:
      show_root_heading: true

::: olgadoc.ExtractedImage
    options:
      show_root_heading: true

## Processability

::: olgadoc.HealthLabel
    options:
      show_root_heading: true

::: olgadoc.HealthIssueKind
    options:
      show_root_heading: true

::: olgadoc.HealthIssue
    options:
      show_root_heading: true

::: olgadoc.HealthIssueSimple
    options:
      show_root_heading: true

::: olgadoc.HealthIssueApproximatePagination
    options:
      show_root_heading: true

::: olgadoc.HealthIssueHeuristicStructure
    options:
      show_root_heading: true

::: olgadoc.HealthIssueCounted
    options:
      show_root_heading: true

## Format discrimination

::: olgadoc.FormatName
    options:
      show_root_heading: true

::: olgadoc.FormatHint
    options:
      show_root_heading: true
