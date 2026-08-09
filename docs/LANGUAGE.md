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

There are exactly fifteen. No others are accepted.

| Tag | Role | Allowed children |
|---|---|---|
| `ue-email` | Document root | `ue-layout` |
| `ue-layout` | Page container, sets max width | `ue-row`, `ue-hero`, `ue-divider`, `ue-spacer` |
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
| `ue-hero` | Banner with a background image | content tags |
| `ue-bold` | Inline bold, mid-sentence | text, `ue-italic` |
| `ue-italic` | Inline italic, mid-sentence | text, `ue-bold` |

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
| `ue-email` | `lang`, `dark-mode`, `font-family` |
| `ue-layout` | `background`, `background-light`, `background-dark`, `padding`, `margin`, `max-width` |
| `ue-row` | `background`, `background-light`, `background-dark`, `padding`, `border`, `border-radius`, `align`, `gap`, `stack-on="mobile"` |
| `ue-col` | `background`, `background-light`, `background-dark`, `padding`, `border`, `border-radius`, `align` |
| `ue-heading` | `level` *(required)*, `color`, `color-light`, `color-dark`, `font-size`, `align` |
| `ue-text` | `color`, `color-light`, `color-dark`, `font-size`, `line-height`, `align` |
| `ue-button` | `href` *(required)*, `background`, `color`, `theme`, `border-radius`, `align`, `accessible-label` |
| `ue-image` | `src` + `alt` *(required)*, `width`, `height`, `border-radius`, `dark-src` |
| `ue-divider` | `color`, `thickness`, `margin` |
| `ue-spacer` | `height` |
| `ue-hero` | `src` *(required)*, `background`, `width`, `height`, `padding`, `align` |

`background` and `background-light` are interchangeable everywhere.

### Cards

`ue-col` carries its own box styling — that is how you build the tiles,
callout boxes and partner blocks that make up most real marketing emails:

```html
<ue-row gap="16px" stack-on="mobile">
  <ue-col background="#FFFFFF" padding="20px" border-radius="12px" align="center">
    <ue-image src="coins.png" alt="Savings" width="64px" />
    <ue-text align="center">Cut your costs</ue-text>
  </ue-col>
  <ue-col background="#FFFFFF" padding="20px" border-radius="12px" align="center">
    <ue-image src="rocket.png" alt="Fast" width="64px" />
    <ue-text align="center">Set up in minutes</ue-text>
  </ue-col>
</ue-row>
```

A column background is emitted twice — as CSS and as a `bgcolor` attribute —
because Outlook's Word engine honours the attribute far more reliably.

### Font

`font-family` on `ue-email` sets the typeface for the whole document. It is
emitted on **every** text block rather than once on `<body>`, because Outlook's
Word engine does not inherit fonts into tables — and the entire layout is
tables.

Without it, a sans-serif stack is used. Emitting nothing was the previous
behaviour, and left Outlook rendering a serif: no branded email ever looked
like its mockup.

Use fonts that are actually installed on the reader's machine. A webfont does
not load in most email clients, so list a real fallback:

```html
<ue-email font-family="Inter, Helvetica, Arial, sans-serif">
```

### Columns and `stack-on`

Columns are laid out side by side in every client. `stack-on="mobile"` stacks
them through a media query, at the reader's actual window width.

A client that does not support media queries — Outlook Desktop — **keeps the
columns side by side**. It used to stack them permanently, which broke every
two-column layout there: a product grid or an image-left/text-right block came
out one item per row. Outlook Desktop is a *desktop* client; it is never the
mobile case, and it renders table columns perfectly.

### Spacing between columns

`gap` on `ue-row` works everywhere. Where flexbox is available it uses the
native property; elsewhere the compiler inserts real spacer cells, because a
table cell has no notion of `gap` — the attribute used to vanish for Gmail and
Outlook, and the cards touched.

With `stack-on="mobile"`, the spacers are hidden once the row stacks: kept,
they would become empty bands between the cards.

### Background images

`ue-hero` puts content **on top of** a background image. It sits directly
under `ue-layout` and carries its children itself — no `ue-row`/`ue-col`.

```html
<ue-hero src="https://cdn.example.com/hero.jpg" background="#0F1B33" height="420px">
  <ue-heading level="1" color="#FFFFFF" align="center">A headline over the image</ue-heading>
  <ue-button href="https://example.com" align="center">Get started</ue-button>
</ue-hero>
```

A background image in email is not a CSS property you set and forget. Outlook
ignores `background-image`, and a large share of recipients block images
entirely. Three mechanisms are emitted, each covering what the previous one
does not:

1. **`bgcolor`** — the fallback colour, and the only thing visible when images
   are blocked. It is what decides whether your text stays readable, so choose
   it against your text colour, not for decoration. Defaults to a dark grey;
   set `background` yourself.
2. **the HTML `background` attribute and the CSS declaration** — some clients
   honour one, some the other.
3. **a VML rectangle** for Outlook, with the content re-injected inside a
   `v:textbox`. It is the only technique that puts text over an image in the
   Word engine.

VML knows neither percentages nor automatic sizing, so `width` and `height`
must be pixel values — they default to 600×400.

### Inline emphasis

`ue-bold` and `ue-italic` apply to a fragment **inside** a sentence, which is
what real copy needs — an attribute on `ue-text` could only bold the whole
paragraph:

```html
<ue-text>And what if your <ue-bold>next subscription</ue-bold> were your own?</ue-text>
```

They nest, and work inside `ue-heading` and `ue-button` too. They compile to
`<strong>` and `<em>` rather than `<b>` and `<i>`: same rendering everywhere
including Outlook's Word engine, and the meaning survives for screen readers.

They are only valid where text is written — directly under `ue-col` they are
rejected, because there is no sentence to emphasise.

### Alignment

`align` on `ue-col` centres everything inline inside it: text, headings,
images. A **button is a `<table>`**, which is block-level, so `text-align`
does not centre it — put `align="center"` on the `ue-button` itself.

### Image width

`width` accepts any CSS length. A pixel value is also emitted as the HTML
`width` attribute — as a **bare integer**, since `width="160px"` is invalid and
gets ignored, which leaves Outlook showing the image at its native size. A
relative value (`100%`, `auto`) stays CSS-only rather than being guessed at.

Every image gets `max-width:100%` and, unless you set `height` yourself,
`height:auto`. A fixed pixel width alone overflowed the screen on mobile: the
content got cut off, or the client zoomed the whole email out.

### Rounded corners

`border-radius` works on `ue-col`, `ue-row`, `ue-button` and `ue-image`.

Outlook's Word engine ignores CSS `border-radius`: corners stay square there,
while the background and padding still apply. The **button** is the exception —
its Outlook fallback is VML, whose `arcsize` is derived from your radius, so a
pill button stays a pill. The button is 44px tall, so any radius from 22px up
renders fully rounded.

### Dark mode

Any attribute with a `-light` / `-dark` pair is switched by the email client:

```html
<ue-layout background-light="#FFFFFF" background-dark="#0F1B33">
```

`ue-image` has `dark-src` for the same purpose. Both images are emitted and
toggled by the media query — not a `<picture>` element, which Gmail strips and
Outlook ignores, and which would therefore never show the dark variant.

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
