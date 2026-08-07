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
