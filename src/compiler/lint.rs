//! Linter complet (Obsidian_Unimail/audit-uetl-compiler.md, étape 9) —
//! avertissements non bloquants sur l'AST déjà analysé. Le parseur rejette
//! ce qui casse la compilation (balise inconnue, hiérarchie invalide,
//! attribut requis manquant) ; ce module signale ce qui compile mais ne
//! fera pas ce que l'auteur attend, silencieusement :
//!
//! - un attribut que la balise ne lit jamais (`unknown_attribute`) ;
//! - une valeur d'énumération qui retombe sur un défaut sans un mot
//!   (`invalid_value`, ex: `theme="danget"` → thème primaire, en silence) ;
//! - une longueur CSS mal formée (`invalid_value`, ex: `padding="20 px"`) ;
//! - une URL au schéma refusé, réécrite en `#` à la compilation
//!   (`unsafe_url_scheme`) ;
//! - des largeurs de colonnes dont la somme dépasse 100% (`column_width_overflow`).
//!
//! Un template (`{{ expr }}`) n'est jamais signalé par les trois derniers :
//! sa valeur réelle n'est connue qu'à l'envoi, impossible à valider ici.

use crate::parser::ast::{AttrValue, DocumentNode, ElementNode, Node, UetlTag};

pub fn collect_warnings(doc: &DocumentNode) -> Vec<String> {
    let mut warnings = Vec::new();
    for child in &doc.children {
        walk_node(child, &mut warnings);
    }
    warnings
}

fn walk_node(node: &Node, warnings: &mut Vec<String>) {
    if let Node::Element(el) = node {
        check_unknown_attributes(el, warnings);
        check_enum_values(el, warnings);
        check_length_values(el, warnings);
        check_unsafe_url_schemes(el, warnings);
        if el.tag == UetlTag::Row {
            check_column_width_budget(el, warnings);
        }
        for child in &el.children {
            walk_node(child, warnings);
        }
    }
}

fn push(warnings: &mut Vec<String>, el: &ElementNode, message: String) {
    warnings.push(format!(
        "{message} (line {}, column {})",
        el.span.line, el.span.column
    ));
}

// ── Attribut inconnu ─────────────────────────────────────

fn check_unknown_attributes(el: &ElementNode, warnings: &mut Vec<String>) {
    let known = el.tag.known_attributes();
    for name in el.attrs.keys() {
        if !known.contains(&name.as_str()) {
            push(
                warnings,
                el,
                format!("unknown_attribute: '{name}' on {}", el.tag.tag_name()),
            );
        }
    }
}

// ── Valeurs d'énumération ────────────────────────────────

const ALIGN_VALUES: &[&str] = &["left", "center", "right", "justify"];

fn check_enum_values(el: &ElementNode, warnings: &mut Vec<String>) {
    // `align` apparaît sur la plupart des balises stylables — vérifié
    // partout où il est présent, pas seulement sur une liste de balises.
    check_enum_attr(el, "align", ALIGN_VALUES, warnings);

    match el.tag {
        UetlTag::Email => check_enum_attr(el, "dark-mode", &["auto", "manual", "off"], warnings),
        UetlTag::Row => check_enum_attr(el, "stack-on", &["mobile"], warnings),
        UetlTag::Button => {
            check_enum_attr(el, "theme", &["primary", "secondary", "danger"], warnings)
        }
        _ => {}
    }
}

fn check_enum_attr(el: &ElementNode, name: &str, allowed: &[&str], warnings: &mut Vec<String>) {
    if let Some(value) = string_attr(el, name) {
        if !allowed.contains(&value) {
            push(
                warnings,
                el,
                format!(
                    "invalid_value: '{name}' on {} has value '{value}', expected one of {allowed:?}",
                    el.tag.tag_name()
                ),
            );
        }
    }
}

// ── Longueurs CSS ────────────────────────────────────────

/// Attributs cense contenir une longueur CSS simple — `border` en est
/// exclu : c'est un raccourci composite ("1px solid #ccc"), pas une seule
/// longueur, et le valider demanderait un tout autre analyseur.
const LENGTH_ATTRS: &[&str] = &[
    "padding",
    "font-size",
    "border-radius",
    "margin",
    "gap",
    "height",
    "width",
    "thickness",
    "line-height",
];

fn check_length_values(el: &ElementNode, warnings: &mut Vec<String>) {
    for name in LENGTH_ATTRS {
        if let Some(value) = string_attr(el, name) {
            if !looks_like_css_length(value) {
                push(
                    warnings,
                    el,
                    format!(
                        "invalid_value: '{name}' on {} has value '{value}', expected a CSS length (e.g. '20px', '50%', or a bare number)",
                        el.tag.tag_name()
                    ),
                );
            }
        }
    }
}

/// Un nombre nu, ou un nombre suivi de `px`/`%`/`em`/`rem` — sans espace
/// entre les deux (`"20 px"` est aussi invalide en CSS que `"20pxx"`).
/// `line-height` accepte legitimement un nombre nu comme multiplicateur
/// (`"1.5"`), d'ou l'unite vide autorisee pour tous, pas seulement pour lui.
fn looks_like_css_length(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }

    for unit in ["px", "%", "em", "rem"] {
        if let Some(number) = value.strip_suffix(unit) {
            return !number.is_empty() && number.parse::<f64>().is_ok();
        }
    }

    value.parse::<f64>().is_ok()
}

// ── Schémas d'URL non sûrs ───────────────────────────────

// Doit rester synchronisée avec `ALLOWED_SCHEMES` dans
// `html_gen.rs::attr_url` — dupliquée plutôt que partagée pour ne pas
// exposer un détail interne du générateur HTML au seul usage du linter ;
// le prix est une petite liste à tenir à jour à deux endroits.
const ALLOWED_URL_SCHEMES: [&str; 4] = ["https:", "http:", "mailto:", "tel:"];

fn check_unsafe_url_schemes(el: &ElementNode, warnings: &mut Vec<String>) {
    let url_attr = match el.tag {
        UetlTag::Button => Some("href"),
        UetlTag::Image | UetlTag::Hero => Some("src"),
        UetlTag::Interactive => Some("fallback-src"),
        _ => None,
    };

    let Some(attr_name) = url_attr else { return };
    let Some(value) = string_attr(el, attr_name) else {
        return;
    };

    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    let has_scheme = lower.contains(':');
    let is_allowed = ALLOWED_URL_SCHEMES.iter().any(|s| lower.starts_with(s));

    if has_scheme && !is_allowed {
        push(
            warnings,
            el,
            format!(
                "unsafe_url_scheme: '{attr_name}' on {} will be replaced with '#' at compile time \
                 (only https:, http:, mailto:, tel:, or a {{{{ template }}}} placeholder are allowed)",
                el.tag.tag_name()
            ),
        );
    }
}

// ── Budget de largeur des colonnes ───────────────────────

fn check_column_width_budget(row: &ElementNode, warnings: &mut Vec<String>) {
    let total: f64 = row
        .children
        .iter()
        .filter_map(|c| match c {
            Node::Element(col) if col.tag == UetlTag::Col => string_attr(col, "width"),
            _ => None,
        })
        .filter_map(|w| {
            w.strip_suffix('%')
                .and_then(|n| n.trim().parse::<f64>().ok())
        })
        .sum();

    if total > 100.0 {
        push(
            warnings,
            row,
            format!("column_width_overflow: this row's columns add up to {total}%, which overflows the available width"),
        );
    }
}

// ── Aides ─────────────────────────────────────────────────

/// `None` pour un attribut absent, booléen (jamais produit par le
/// parseur aujourd'hui — voir `AttrValue`), ou template : ces trois cas
/// ne peuvent pas être validés par les règles de ce module.
fn string_attr<'a>(el: &'a ElementNode, name: &str) -> Option<&'a str> {
    match el.attrs.get(name) {
        Some(AttrValue::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn warnings_for(src: &str) -> Vec<String> {
        let doc = Parser::parse_document(src).unwrap();
        collect_warnings(&doc)
    }

    #[test]
    fn warns_on_an_attribute_the_tag_never_reads() {
        // `margin` sur `ue-layout` : documenté dans LANGUAGE.md, mais jamais
        // lu par `gen_layout` (html_gen.rs) — un vrai cas actuel, pas un
        // exemple inventé.
        let warnings = warnings_for(
            r#"<ue-email><ue-layout margin="20px"><ue-row><ue-col><ue-text>hi</ue-text></ue-col></ue-row></ue-layout></ue-email>"#,
        );

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unknown_attribute"));
        assert!(warnings[0].contains("'margin'"));
        assert!(warnings[0].contains("ue-layout"));
    }

    #[test]
    fn no_warning_for_a_fully_recognized_document() {
        let warnings = warnings_for(
            r##"<ue-email lang="fr"><ue-layout padding="20"><ue-row><ue-col background="#fff"><ue-heading level="2">Hi</ue-heading><ue-text color="#333">Body</ue-text></ue-col></ue-row></ue-layout></ue-email>"##,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn catches_a_typo_deep_in_the_tree() {
        let warnings = warnings_for(
            r##"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://a.example" backgroundd="#fff">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"##,
        );

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("'backgroundd'"));
        assert!(warnings[0].contains("ue-button"));
    }

    #[test]
    fn a_raw_block_accepts_no_attributes_at_all() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row><ue-col><ue-raw class="whatever">html</ue-raw></ue-col></ue-row></ue-layout></ue-email>"#,
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("ue-raw"));
    }

    #[test]
    fn an_unrecognized_theme_is_flagged_instead_of_silently_falling_back() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="https://a.example" theme="danget">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#,
        );

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("invalid_value"));
        assert!(warnings[0].contains("'theme'"));
        assert!(warnings[0].contains("danget"));
    }

    #[test]
    fn an_unrecognized_stack_on_value_is_flagged() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row stack-on="tablet"><ue-col><ue-text>hi</ue-text></ue-col></ue-row></ue-layout></ue-email>"#,
        );

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("'stack-on'"));
    }

    #[test]
    fn an_invalid_align_value_is_flagged_on_any_tag_that_carries_it() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row><ue-col align="middle"><ue-text>hi</ue-text></ue-col></ue-row></ue-layout></ue-email>"#,
        );

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("'align'"));
        assert!(warnings[0].contains("middle"));
    }

    #[test]
    fn a_malformed_length_is_flagged() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row><ue-col padding="20 px"><ue-text>hi</ue-text></ue-col></ue-row></ue-layout></ue-email>"#,
        );

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("invalid_value"));
        assert!(warnings[0].contains("'padding'"));
    }

    #[test]
    fn a_bare_number_line_height_is_valid_not_a_length_error() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row><ue-col><ue-text line-height="1.5">hi</ue-text></ue-col></ue-row></ue-layout></ue-email>"#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_javascript_href_is_flagged_as_an_unsafe_scheme() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="javascript:alert(1)">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#,
        );

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unsafe_url_scheme"));
        assert!(warnings[0].contains("'href'"));
    }

    #[test]
    fn a_templated_href_is_never_flagged() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row><ue-col><ue-button href="{{cta_url}}">Go</ue-button></ue-col></ue-row></ue-layout></ue-email>"#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn columns_summing_past_a_hundred_percent_are_flagged() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row><ue-col width="70%"><ue-text>A</ue-text></ue-col><ue-col width="40%"><ue-text>B</ue-text></ue-col></ue-row></ue-layout></ue-email>"#,
        );

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("column_width_overflow"));
    }

    #[test]
    fn columns_within_budget_are_not_flagged() {
        let warnings = warnings_for(
            r#"<ue-email><ue-layout><ue-row><ue-col width="62%"><ue-text>A</ue-text></ue-col><ue-col width="36%"><ue-text>B</ue-text></ue-col></ue-row></ue-layout></ue-email>"#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }
}
