# Common delivery export

`POST /compile/send` accepts `{ "uetl": "<ue-email>...</ue-email>" }` and
returns the existing `{ "html": "...", "warnings": [] }` compilation shape.
Deploy this compiler before switching a CRM worker to this route. A missing
route is an integration failure; do not fall back silently to Gmail previews.

The `universal-2026-11-04` contract is exposed by `/health`. It is separate
from `/profiles` and `/compile/all`, which still describe seven diagnostic
profiles, not seven native-client renderers.

The delivery export uses table layouts, responsive CSS, declared dark-mode
rules, conditional MSO width wrappers and VML hero backgrounds. Layout colors
have an HTML bgcolor fallback. Buttons use cell padding and flexible dimensions,
with `data-uetl-cta="true"` for the CRM's post-personalization guard. Outlook may
show square corners; fixed 200×44 VML buttons remain in the explicit Outlook
preview but are not used for common delivery. Hero VML still uses explicit
height: long/dynamic content needs review and native-client testing.

The language attribute is escaped in all exports. No source-language inference
or automatic dark palette is performed. Conditional branches, unknown support,
and simulations cannot guarantee identical native rendering or inbox placement.

Regression tests cover long/dynamic CTA labels, MSO hero/width branches,
responsive/dark CSS, bgcolor, invalid input, deterministic output, legacy
profile contracts and language-attribute injection. Real inbox captures are
not provided by these tests.
