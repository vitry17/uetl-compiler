# UETL Language Reference

The authoritative description of what the parser accepts. Extracted from
`src/parser/ast.rs` (tags, hierarchy) and `src/parser/parser.rs` (required
attributes) — if this document and the compiler ever disagree, the compiler
wins and this file is the bug.

## Why this document exists

An LLM asked to generate a UETL template with no reference will invent a
plausible-looking syntax — `<uetl>`, `<html>`, `<mjml>` — and be wrong. So will
a new contributor. Every tag below is rejected by the parser unless it appears
in this list, so the list is worth keeping accurate.

## Document structure

A document is a strict hierarchy. The root **must** be `<ue-email>`; anything
else fails immediately with `root element must be '<ue-email>', found '<...>'`.

```
ue-email
└── ue-layout
    ├── ue-row
    │   └── ue-col
    │       ├── ue-heading
    │       ├── ue-text
    │       ├── ue-image
    │       ├── ue-button
    │       ├── ue-divider
    │       ├── ue-spacer
    │       ├── ue-interactive
    │       ├── ue-raw
    │       └── ue-row          (nested rows are allowed)
    ├── ue-divider
    └── ue-spacer
```

Minimal valid document:

```html
<ue-email lang="en">
  <ue-layout>
    <ue-row>
      <ue-col>
        <ue-text>Hello.</ue-text>
      </ue-col>
    </ue-row>
  </ue-layout>
</ue-email>
```

## Tags

There are exactly twelve. No others are accepted.

| Tag | Role | Allowed children |
|---|---|---|
| `ue-email` | Document root | `ue-layout` |
| `ue-layout` | Page container, sets max width | `ue-row`, `ue-divider`, `ue-spacer` |
| `ue-row` | Horizontal band | `ue-col` |
| `ue-col` | Column, holds the content | all content tags, plus `ue-row` |
| `ue-heading` | Heading | text |
| `ue-text` | Paragraph | text |
| `ue-button` | Call to action | text |
| `ue-image` | Image | — |
| `ue-divider` | Horizontal rule | — |
| `ue-spacer` | Vertical space | — |
| `ue-interactive` | Interactive block (AMP-style) | content tags |
| `ue-raw` | Escape hatch for raw HTML | raw content |

## Required attributes

Omitting these fails parsing with `missing required attribute`.

- `ue-button` — **`href`**
- `ue-image` — **`src`** and **`alt`**
- `ue-heading` — **`level`**, an integer from 1 to 6 (a `{{ variable }}` is also
  accepted, since the level may be computed at send time)

All other attributes are optional and passed through to the renderer.

## Styling attributes

There is no `<style>` tag and no CSS. Everything is an attribute, and only the
ones listed here are read — anything else is silently ignored, which is the
usual reason a template comes out unstyled.

| Tag | Attributes |
|---|---|
| `ue-email` | `lang`, `dark-mode` |
| `ue-layout` | `background`, `background-light`, `background-dark`, `padding`, `margin`, `max-width` |
| `ue-row` | `background`, `padding`, `gap`, `stack-on="mobile"` |
| `ue-col` | none — it only holds content, style the row instead |
| `ue-heading` | `level` *(required)*, `color`, `color-light`, `color-dark`, `font-size`, `align` |
| `ue-text` | `color`, `color-light`, `color-dark`, `font-size`, `line-height` |
| `ue-button` | `href` *(required)*, `background`, `color`, `theme`, `accessible-label` |
| `ue-image` | `src` + `alt` *(required)*, `width`, `height`, `dark-src` |
| `ue-divider` | `color`, `thickness`, `margin` |
| `ue-spacer` | `height` |

### Dark mode

Any attribute with a `-light` / `-dark` pair is switched by the email client:

```html
<ue-layout background-light="#FFFFFF" background-dark="#0F1B33">
```

`ue-image` has `dark-src` for the same purpose.

### Button colour

`background` and `color` take any colour and **override** the `theme` preset:

```html
<ue-button href="..." background="#00AFF5" color="#05073B">Go</ue-button>
```

`theme` remains a shortcut when no brand colour applies:

| `theme` | Background | Text |
|---|---|---|
| `primary` (default) | `#2E5FAC` | `#ffffff` |
| `secondary` | `#6c757d` | `#ffffff` |
| `danger` | `#d9534f` | `#ffffff` |

Both work across every profile, including the Outlook VML fallback.

## Template variables

`{{ variable }}` is valid inside text content and inside attribute values. The
parser treats it as an opaque token and does not evaluate it — substitution
happens later, at send time.

```html
<ue-heading level="1">Welcome, {{ contact.firstname }}</ue-heading>
<ue-button href="{{ params.cta_url }}">Open</ue-button>
```

## Validation endpoint

`POST /validate` with `{"uetl": "<source>"}` always answers **HTTP 200**:

```json
{ "valid": false, "errors": ["root element must be '<ue-email>', found '<uetl>' (line 1, column 1)"], "warnings": [] }
```

The status code says the validation *ran*, not that the source is good — read
the `valid` field. This distinction is deliberate: it lets a caller separate
"your source has errors" (200, `valid: false`) from "the compiler is down"
(5xx), which matters for an editor doing validation on every keystroke.

`POST /compile` and `POST /compile-all` behave differently: an invalid source
there is **HTTP 422** with `{"error": {"code": "parse_error", "message": "..."}}`,
because compilation genuinely cannot produce a result.

## Error messages

Parse errors carry the tag, the line and the column, and say what was expected.
They are meant to be shown to the user verbatim rather than replaced by a
generic message — they are the only thing that says what to fix.

```
root element must be '<ue-email>', found '<uetl>' (line 1, column 1)
unknown tag '<div>' (line 4, column 7)
'<ue-text>' is not a valid child of '<ue-layout>' (line 3, column 5)
'<ue-col>' was never closed (line 8, column 3)
missing required attribute 'href' on '<ue-button>' (line 6, column 9)
```
