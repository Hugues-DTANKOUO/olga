# JSON tree

`Document.to_json()` returns a full structural tree of the document.
Every node is discriminated on its `type` literal, so `mypy --strict`
narrows exactly one variant per branch.

## Top-level

::: olgadoc.DocumentJson
    options:
      show_root_heading: true

::: olgadoc.JsonSource
    options:
      show_root_heading: true

::: olgadoc.JsonPageInfo
    options:
      show_root_heading: true

::: olgadoc.JsonWarning
    options:
      show_root_heading: true

::: olgadoc.JsonBBox
    options:
      show_root_heading: true

## Discriminated element union

::: olgadoc.JsonElement
    options:
      show_root_heading: true

::: olgadoc.JsonElementType
    options:
      show_root_heading: true

## Element variants

::: olgadoc.JsonDocumentElement
    options:
      show_root_heading: true

::: olgadoc.JsonSectionElement
    options:
      show_root_heading: true

::: olgadoc.JsonHeadingElement
    options:
      show_root_heading: true

::: olgadoc.JsonParagraphElement
    options:
      show_root_heading: true

::: olgadoc.JsonTableElement
    options:
      show_root_heading: true

::: olgadoc.JsonTableRowElement
    options:
      show_root_heading: true

::: olgadoc.JsonTableCellElement
    options:
      show_root_heading: true

::: olgadoc.JsonTableCellDetail
    options:
      show_root_heading: true

::: olgadoc.JsonListElement
    options:
      show_root_heading: true

::: olgadoc.JsonListItemElement
    options:
      show_root_heading: true

::: olgadoc.JsonImageElement
    options:
      show_root_heading: true

::: olgadoc.JsonCodeBlockElement
    options:
      show_root_heading: true

::: olgadoc.JsonBlockQuoteElement
    options:
      show_root_heading: true

::: olgadoc.JsonPageHeaderElement
    options:
      show_root_heading: true

::: olgadoc.JsonPageFooterElement
    options:
      show_root_heading: true

::: olgadoc.JsonFootnoteElement
    options:
      show_root_heading: true

::: olgadoc.JsonAlignedLineElement
    options:
      show_root_heading: true

::: olgadoc.JsonSpan
    options:
      show_root_heading: true
