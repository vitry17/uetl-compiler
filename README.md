# UETL Compiler

A Rust compiler that transforms **UETL** (Universal Email Templating Language) — a small, semantic markup language for emails — into cross-client HTML, with a different rendering strategy per email client (Gmail, Outlook Desktop, Outlook 365, Apple Mail, Yahoo Mail, Thunderbird, Samsung Mail).

## Why

Email HTML in 2026 still looks like 1999: nested tables, inline styles, MSO conditional comments, VML hacks for Outlook buttons. [MJML](https://mjml.io) made this more bearable, but its client profiles are hardcoded in the compiler itself, and its governance lives entirely inside one company.

UETL takes the same idea — write semantic components, compile to compatible HTML — but with client capabilities expressed as **data** (JSON profiles) rather than baked into the code. Want to tweak how Outlook 365 handles `position: absolute`? Edit a JSON file, not the Rust source.

```html
<ue-email lang="en">
  <ue-layout max-width="600px">
    <ue-row>
      <ue-col>
        <ue-button href="https://example.com" theme="primary">Get started</ue-button>
      </ue-col>
    </ue-row>
  </ue-layout>
</ue-email>
```

compiles to a VML `<v:roundrect>` + table fallback for Outlook Desktop, and a plain styled `<table><a>` for everyone else — same source, different output, chosen automatically from the target profile.

## Status

Early but functional: lexer, recursive-descent parser with semantic validation, HTML generator, and an HTTP API all work end-to-end with 50 passing tests. Dark mode is supported on headings, text, layout backgrounds, and images. What's missing: a CLI/visual preview tool, AMP-style interactivity, and a published crate. Contributions and bug reports on real-world rendering quirks are very welcome.

## Quickstart

```bash
cargo test            # 50 tests: lexer, parser, profiles, html generator, HTTP API
cargo run             # serves on :4001
```

Or with Docker:

```bash
docker build -t uetl-compiler .
docker run -p 4001:4001 uetl-compiler
```

## Performance

```bash
cargo bench
```

On a representative email (logo, responsive two-column section, button, dark-mode image), measured on a regular dev machine:

| Benchmark                     | Time      |
|--------------------------------|-----------|
| Parse UETL → AST               | ~25 µs    |
| Compile AST → HTML (1 profile) | ~17 µs    |
| Compile for all 7 profiles     | ~136 µs   |

Comfortably under the 50ms/request target — there's room to add real-world complexity before this becomes a bottleneck.

## API

| Method | Route          | Body                          | Description                                  |
|--------|----------------|--------------------------------|-----------------------------------------------|
| GET    | `/health`      | —                              | Liveness check                                |
| GET    | `/profiles`    | —                              | List of available client profile IDs          |
| POST   | `/validate`    | `{ "uetl": "..." }`            | Parse without compiling; returns errors if any|
| POST   | `/compile`     | `{ "uetl": "...", "client": "gmail" }` | Compile for one client                |
| POST   | `/compile/all` | `{ "uetl": "..." }`            | Compile for every known client at once        |

```bash
curl -X POST localhost:4001/compile \
  -H 'content-type: application/json' \
  -d '{"uetl": "<ue-email><ue-layout><ue-row><ue-col><ue-text>Hi</ue-text></ue-col></ue-row></ue-layout></ue-email>", "client": "gmail"}'
```

## Supported clients

Each client is a JSON profile under `src/profiles/`, describing CSS support (`full` / `partial` / `none`) and quirks (e.g. `vml_support` for Outlook's Word rendering engine). Currently shipped: `gmail`, `outlook_desktop`, `outlook_365`, `apple_mail`, `yahoo_mail`, `thunderbird`, `samsung_mail`.

## Components

| Tag             | Required attrs | Key optional attrs                                  |
|-----------------|-----------------|-------------------------------------------------------|
| `<ue-email>`    | —               | `lang`, `dark-mode="auto"`                            |
| `<ue-layout>`   | —               | `max-width`, `background-light`/`background-dark`, `padding` |
| `<ue-row>`      | —               | `stack-on="mobile"`, `gap`, `background`, `padding`   |
| `<ue-col>`      | —               | — (groups content inside a `<ue-row>`)                |
| `<ue-heading>`  | `level` (1–6)   | `color-light`/`color-dark`, `font-size`, `align`      |
| `<ue-text>`     | —               | `color-light`/`color-dark`, `font-size`, `line-height`|
| `<ue-button>`   | `href`          | `theme`, `accessible-label`                           |
| `<ue-image>`    | `src`, `alt`    | `width`, `height`, `dark-src`                         |
| `<ue-divider>`  | —               | `color`, `thickness`, `margin`                        |
| `<ue-spacer>`   | —               | `height` (default `20px`)                             |
| `<ue-raw>`      | —               | embeds literal HTML untouched (escape hatch)           |

Any attribute value can be a template token, e.g. `href="{{cta_url}}"` — it's preserved as-is in the compiled output for the calling backend to substitute.

## Compared to MJML

| | MJML | UETL Compiler |
|---|---|---|
| Output | One HTML for all clients | Per-client optimized HTML |
| Client capabilities | Hardcoded in the compiler | JSON profiles, editable without touching Rust |
| Dark mode | Manual media queries | `color-dark`/`background-dark` attrs, compiled automatically |
| Governance | Mailgun (private company) | Source-available (BSL 1.1 → Apache 2.0 in 2030) |

## Architecture

```
UETL source
  → Lexer       (src/lexer)     tokens with line/column tracking
  → Parser      (src/parser)    AST + semantic validation (e.g. <ue-col> only inside <ue-row>)
  → HtmlGenerator (src/compiler) per-component rendering strategy, driven by the target Profile
```

The compiler has no business logic — it receives UETL, returns HTML. No database, no auth, no email sending. It's meant to be called from whatever backend orchestrates contacts, campaigns, and sending.

## Contributing

Bug reports on real client rendering (with the UETL source, target client, and screenshot) are the most valuable contributions right now. PRs adding or correcting a client profile are also very welcome.

## License

[Business Source License 1.1](LICENSE) — free to read, modify, and use for any purpose, including internal commercial use. The only thing it restricts is reselling or hosting the compiler's functionality as a competing service to third parties; that requires a commercial license from the Licensor. On 2030-06-30, this license automatically converts to Apache License 2.0 and the project becomes fully open source with no restrictions.
