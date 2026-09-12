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

Early but functional: lexer, recursive-descent parser with semantic validation, HTML generator, and an HTTP API all work end-to-end with over 150 passing tests. Dark mode is supported on headings, text, layout backgrounds, and images. Attribute values are HTML-escaped and `href`/`src` are restricted to safe URL schemes — templates are user-authored and previewed in an iframe before sending, so this isn't optional. `/compile` and `/validate` also return `warnings` for attributes a tag never reads, the most common reason a template comes out unstyled, plus structured `diagnostics` with real line/column for editor integration. Parsing is bounded (max nesting depth, 256KB source cap) and runs off the async event loop; profile JSON is schema-checked at load, `/health` forces that check instead of only catching a broken profile at the first real compile; `/compile/all` output is deterministically ordered. `/validate` recovers from local errors (an unknown tag, a missing required attribute, an invalid child) to report every one found in a document in a single pass, instead of only the first — `/compile`/`/compile/all` still fail fast on the first error, since generating HTML from a tree with silently-dropped elements would be worse than refusing to compile it. What's missing: a CLI/visual preview tool, real AMP-for-Email content injection (`ue-interactive` today only renders a static fallback image or text), and a published crate. Contributions and bug reports on real-world rendering quirks are very welcome.

## Language reference

**[docs/LANGUAGE.md](docs/LANGUAGE.md)** — the fifteen tags, the hierarchy the
parser enforces, required attributes, and the exact error messages. Worth
reading before writing a template by hand, and worth pointing any code
generator at: without it, a plausible-looking `<uetl>` or `<div>` gets written
and rejected.

## Quickstart

```bash
cargo test            # lexer, parser, profiles, html generator, security, determinism, HTTP API
cargo run             # serves on :4001 (binds 127.0.0.1 by default, see Configuration)
```

Or with Docker:

```bash
docker build -t uetl-compiler .
docker run -p 4001:4001 -e COMPILER_BIND=0.0.0.0:4001 uetl-compiler
```

## Configuration

All optional, read once at startup:

| Variable                          | Default            | Purpose                                             |
|------------------------------------|---------------------|------------------------------------------------------|
| `COMPILER_BIND`                    | `127.0.0.1:4001`    | Listen address — set to `0.0.0.0:4001` in a container so other containers/the host can reach it |
| `COMPILER_CORS_ORIGIN`             | none (no CORS)      | A single allowed origin, or `*`. Only relevant if a browser ever calls this API directly — the intended caller is a backend, not a browser |
| `COMPILER_RATE_LIMIT_PER_SECOND`   | `50`                | Global (not per-caller) request cap, a guard against a runaway loop rather than a capacity limit |

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
| GET    | `/health`      | —                              | Liveness check — also loads and schema-validates every bundled profile, so a broken profile fails this instead of the first real compile |
| GET    | `/profiles`    | —                              | `{ "profiles": [{ "id", "name", "version" }, ...] }`, sorted by id |
| POST   | `/validate`    | `{ "uetl": "..." }`            | Parse without compiling; error-tolerant — reports every local error found (unknown tag, missing required attribute, invalid child, invalid heading level) in one pass as `errors: string[]` plus structured `diagnostics: Diagnostic[]` (code/line/column) |
| POST   | `/compile`     | `{ "uetl": "...", "client": "gmail" }` | Compile for one client                |
| POST   | `/compile/all` | `{ "uetl": "..." }`            | Compile for every known client at once, `results` sorted by client id |

A source over 256KB is rejected with `413` (`source_too_large`) before parsing. `/compile` and `/compile/all` error bodies are `{ "error": { "code", "message", "line", "column", ... } }` — `code` is specific (`unknown_tag`, `missing_required_attr`, `too_deep`, etc.), not a generic `"parse_error"`.

```bash
curl -X POST localhost:4001/compile \
  -H 'content-type: application/json' \
  -d '{"uetl": "<ue-email><ue-layout><ue-row><ue-col><ue-text>Hi</ue-text></ue-col></ue-row></ue-layout></ue-email>", "client": "gmail"}'
```

## Supported clients

Each client is a JSON profile under `src/profiles/`, describing CSS support (`full` / `partial` / `none`) and quirks (e.g. `vml_support` for Outlook's Word rendering engine). Currently shipped: `gmail`, `outlook_desktop`, `outlook_365`, `apple_mail`, `yahoo_mail`, `thunderbird`, `samsung_mail`.

## Components

Fifteen tags. **[docs/LANGUAGE.md](docs/LANGUAGE.md)** is the authoritative,
always-up-to-date reference (attribute lists, hierarchy, error messages) —
this table is a quick-glance summary, kept in sync with it.

| Tag              | Required attrs | Key optional attrs                                              |
|------------------|-----------------|-------------------------------------------------------------------|
| `<ue-email>`     | —               | `lang`, `dark-mode="auto"`, `font-family`, `preview-text`         |
| `<ue-layout>`    | —               | `max-width`, `background`/`-light`/`-dark`, `padding`             |
| `<ue-row>`       | —               | `stack-on="mobile"`, `gap`, `background`, `padding`, `align`      |
| `<ue-col>`       | —               | `background`, `padding`, `border`, `border-radius`, `align`, `width` |
| `<ue-heading>`   | `level` (1–6)   | `color`/`-light`/`-dark`, `font-size`, `align`                    |
| `<ue-text>`      | —               | `color`/`-light`/`-dark`, `font-size`, `line-height`, `align`     |
| `<ue-button>`    | `href`          | `background`, `color`, `theme`, `border-radius`, `padding`, `font-size`, `align`, `accessible-label` |
| `<ue-image>`     | `src`, `alt`    | `width`, `height`, `border-radius`, `dark-src`                    |
| `<ue-divider>`   | —               | `color`, `thickness`, `margin`                                    |
| `<ue-spacer>`    | —               | `height` (default `20px`)                                         |
| `<ue-interactive>` | —             | `fallback-src` — renders a static fallback image; nested tags aren't supported yet |
| `<ue-hero>`      | `src`           | `background`, `width`, `height`, `padding`, `align` — banner with content over a background image |
| `<ue-bold>` / `<ue-italic>` | —    | inline emphasis mid-sentence, nestable, valid inside text/heading/button |
| `<ue-raw>`       | —               | embeds literal HTML untouched, never escaped (escape hatch — see Security) |

Any attribute value can be a template token, e.g. `href="{{cta_url}}"` — it's preserved as-is in the compiled output for the calling backend to substitute. `href`/`src` still get scheme-validated at compile time; the substituted value is the calling backend's responsibility to sanitize.

## Security

Templates are user-authored, previewed in an iframe, and sent to real
recipients — an unescaped attribute or an unrestricted URL scheme is a
stored XSS and a phishing vector, not a theoretical concern. The compiler
therefore:

- HTML-escapes every attribute value before interpolating it (not just
  text content and `accessible-label`).
- Restricts `href`/`src` to `https:`, `http:`, `mailto:`, `tel:`, or a
  `{{ template }}` placeholder (resolved later by the caller) — anything
  else, including `javascript:`/`data:`/`vbscript:`, becomes `#`.
- Treats `<ue-raw>` as what it is: an escape hatch that emits its content
  completely unescaped. Restrict who can author templates using it at the
  platform level — the compiler has no concept of roles or trust.

## Compared to MJML

| | MJML | UETL Compiler |
|---|---|---|
| Output | One HTML for all clients | Per-client optimized HTML |
| Client capabilities | Hardcoded in the compiler | JSON profiles, editable without touching Rust |
| Dark mode | Manual media queries | `color-dark`/`background-dark` attrs, compiled automatically |
| Governance | Mailgun (private company) | Open source (MIT / Apache 2.0) |

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

Dual-licensed under either of [MIT](LICENSE-MIT) or [Apache License, Version 2.0](LICENSE-APACHE), at your option — the convention used across the Rust ecosystem. No restriction on hosting or reselling: the moat, if any, lives in the platform built on top of this compiler, not in the compiler itself.
