/// Étend les couleurs hex courtes (#fff -> #ffffff) ; laisse rgb()/named colors inchangés.
pub fn normalize_color(value: &str) -> String {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let expanded: String = hex.chars().flat_map(|c| [c, c]).collect();
            return format!("#{expanded}");
        }
    }
    value.to_string()
}

/// Ajoute `px` aux valeurs numériques sans unité ("20" -> "20px"); laisse "20%"/"1.5em" inchangés.
pub fn css_unit(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return value.to_string();
    }
    let is_bare_number = value.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-');
    if is_bare_number {
        format!("{value}px")
    } else {
        value.to_string()
    }
}

/// Construit un attribut `style="..."` à partir d'une liste (propriété, valeur optionnelle).
/// Retourne une chaîne vide si aucune déclaration n'est présente.
pub fn style_attr(declarations: &[(&str, Option<String>)]) -> String {
    let body: String = declarations
        .iter()
        .filter_map(|(prop, value)| value.as_deref().map(|v| format!("{prop}:{v};")))
        .collect();
    if body.is_empty() {
        String::new()
    } else {
        format!(" style=\"{body}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_short_hex_colors() {
        assert_eq!(normalize_color("#fff"), "#ffffff");
        assert_eq!(normalize_color("#2E5"), "#22EE55");
    }

    #[test]
    fn leaves_long_hex_and_named_colors_unchanged() {
        assert_eq!(normalize_color("#2E5FAC"), "#2E5FAC");
        assert_eq!(normalize_color("rgb(0,0,0)"), "rgb(0,0,0)");
        assert_eq!(normalize_color("transparent"), "transparent");
    }

    #[test]
    fn adds_px_to_bare_numbers() {
        assert_eq!(css_unit("20"), "20px");
        assert_eq!(css_unit("0"), "0px");
    }

    #[test]
    fn leaves_explicit_units_unchanged() {
        assert_eq!(css_unit("600px"), "600px");
        assert_eq!(css_unit("100%"), "100%");
        assert_eq!(css_unit("1.5em"), "1.5em");
    }

    #[test]
    fn builds_style_attribute_skipping_missing_values() {
        let style = style_attr(&[
            ("color", Some("#fff".to_string())),
            ("font-size", None),
            ("padding", Some("20px".to_string())),
        ]);
        assert_eq!(style, " style=\"color:#fff;padding:20px;\"");
    }

    #[test]
    fn returns_empty_string_when_no_declarations() {
        assert_eq!(style_attr(&[("color", None)]), "");
    }
}
