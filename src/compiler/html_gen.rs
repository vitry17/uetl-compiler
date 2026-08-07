use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Write;

use super::css_inliner::{css_unit, normalize_color, style_attr};
use super::profiles::Profile;
use crate::parser::ast::{AttrValue, DarkModeOption, DocumentNode, ElementNode, Node, UetlTag};

pub struct HtmlGenerator<'a> {
    profile: &'a Profile,
    #[allow(dead_code)]
    indent_level: usize,
    next_id: Cell<usize>,
    /// Règles CSS générées en cours de rendu (media queries de stacking, overrides
    /// dark-mode...), rassemblées dans un unique `<style>` en `<head>` plutôt que
    /// dispersées dans le `<body>` — certains clients (Outlook) ignorent les balises
    /// `<style>` placées hors du `<head>`.
    style_rules: RefCell<Vec<String>>,
}

impl<'a> HtmlGenerator<'a> {
    pub fn generate(ast: &DocumentNode, profile: &'a Profile) -> String {
        let generator = Self {
            profile,
            indent_level: 0,
            next_id: Cell::new(0),
            style_rules: RefCell::new(Vec::new()),
        };
        let body = generator.gen_children(&ast.children);
        generator.gen_email(ast, &body)
    }

    fn next_class(&self, prefix: &str) -> String {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        format!("{prefix}-{id}")
    }

    fn push_style_rule(&self, rule: String) {
        self.style_rules.borrow_mut().push(rule);
    }

    /// Attributs dark-mode à poser sur l'élément, combinant les deux
    /// mécanismes pilotés par l'auteur (indépendants — un profil peut
    /// supporter l'un, l'autre, les deux ou aucun) :
    /// - classe CSS + `@media (prefers-color-scheme:dark)`, pour les clients
    ///   qui exposent réellement ce media query au contenu (`dark_mode_media_query`) ;
    /// - `data-ogsc`/`data-ogsb`, propriétaire Yahoo/AOL Mail (`dark_mode_data_attributes`) :
    ///   ce n'est pas un sélecteur CSS, leur moteur de rendu remplace lui-même
    ///   la couleur affichée par la valeur de cet attribut quand l'utilisateur
    ///   est en mode sombre — ignoré sans effet par tout autre client.
    fn dark_mode_attrs(&self, attrs: &HashMap<String, AttrValue>, attr_name: &str, css_prop: &str) -> String {
        let mut out = String::new();

        if let Some(class) = self.dark_media_class_for_attr(attrs, attr_name, css_prop) {
            let _ = write!(out, " class=\"{class}\"");
        }

        if let Some(data_attr) = self.dark_data_attr_for(attrs, attr_name, css_prop) {
            out.push(' ');
            out.push_str(&data_attr);
        }

        out
    }

    /// Si le profil supporte `prefers-color-scheme` et que l'attribut `*-dark` donné
    /// est présent, enregistre une règle de media query et retourne la classe CSS à
    /// poser sur l'élément. Sinon, ne fait rien.
    fn dark_media_class_for_attr(
        &self,
        attrs: &HashMap<String, AttrValue>,
        attr_name: &str,
        css_prop: &str,
    ) -> Option<String> {
        if !self.profile.supports("dark_mode_media_query").is_supported() {
            return None;
        }
        let value = attr_str(attrs, attr_name)?;
        let value = normalize_color(&value);
        let class = self.next_class("ue-dark");
        self.push_style_rule(format!(
            "@media (prefers-color-scheme:dark){{.{class}{{{css_prop}:{value} !important;}}}}"
        ));
        Some(class)
    }

    fn dark_data_attr_for(&self, attrs: &HashMap<String, AttrValue>, attr_name: &str, css_prop: &str) -> Option<String> {
        if !self.profile.quirk("dark_mode_data_attributes") {
            return None;
        }
        let data_attr_name = match css_prop {
            "color" => "data-ogsc",
            "background" => "data-ogsb",
            _ => return None,
        };
        let value = attr_str(attrs, attr_name)?;
        let value = normalize_color(&value);
        Some(format!("{data_attr_name}=\"{value}\""))
    }

    fn gen_children(&self, children: &[Node]) -> String {
        children.iter().map(|child| self.gen_node(child)).collect()
    }

    fn gen_node(&self, node: &Node) -> String {
        match node {
            Node::Text(text) => html_escape(text),
            Node::Template(expr) => format!("{{{{{expr}}}}}"),
            Node::Element(el) => self.gen_element(el),
            Node::Document(_) => String::new(),
        }
    }

    fn gen_element(&self, el: &ElementNode) -> String {
        match el.tag {
            UetlTag::Email => self.gen_children(&el.children),
            UetlTag::Layout => self.gen_layout(el),
            UetlTag::Row => self.gen_row(el),
            UetlTag::Col => self.gen_col(el),
            UetlTag::Heading => self.gen_heading(el),
            UetlTag::Text => self.gen_text(el),
            UetlTag::Button => self.gen_button(el),
            UetlTag::Image => self.gen_image(el),
            UetlTag::Divider => self.gen_divider(el),
            UetlTag::Spacer => self.gen_spacer(el),
            UetlTag::Interactive => self.gen_interactive(el),
            UetlTag::Raw => self.gen_raw(el),
        }
    }

    /// Coquille HTML du document (doctype, head, meta dark-mode, body).
    fn gen_email(&self, doc: &DocumentNode, body: &str) -> String {
        let dark_meta = if doc.dark_mode == DarkModeOption::Auto
            && self.profile.supports("dark_mode_media_query").is_supported()
        {
            "\n<meta name=\"color-scheme\" content=\"light dark\">\n<meta name=\"supported-color-schemes\" content=\"light dark\">"
        } else {
            ""
        };

        let rules = self.style_rules.borrow();
        let style_tag = if rules.is_empty() {
            String::new()
        } else {
            format!("\n<style>{}</style>", rules.join(""))
        };

        format!(
            "<!DOCTYPE html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">{dark_meta}{style_tag}\n</head>\n<body style=\"margin:0;padding:0;\">\n{body}\n</body>\n</html>",
            lang = doc.lang,
        )
    }

    fn gen_layout(&self, el: &ElementNode) -> String {
        let max_width = attr_str(&el.attrs, "max-width").unwrap_or_else(|| "600px".to_string());
        let padding = attr_str(&el.attrs, "padding").as_deref().map(css_unit);
        let background = attr_str(&el.attrs, "background-light").as_deref().map(normalize_color);

        let outer_style = style_attr(&[("background", background)]);
        let class_attr = self.dark_mode_attrs(&el.attrs, "background-dark", "background");
        let inner_style = style_attr(&[
            ("max-width", Some(max_width)),
            ("margin", Some("0 auto".to_string())),
        ]);
        let cell_style = style_attr(&[("padding", padding)]);

        let rows = self.gen_children(&el.children);

        format!(
            "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"{class_attr}{outer_style}><tr><td align=\"center\">\
<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"{inner_style}><tr><td{cell_style}>{rows}</td></tr></table>\
</td></tr></table>"
        )
    }

    fn gen_row(&self, el: &ElementNode) -> String {
        let gap = attr_str(&el.attrs, "gap").as_deref().map(css_unit);
        let background = attr_str(&el.attrs, "background").as_deref().map(normalize_color);
        let padding = attr_str(&el.attrs, "padding").as_deref().map(css_unit);
        let stack_on_mobile = attr_str(&el.attrs, "stack-on").as_deref() == Some("mobile");

        let cols: Vec<&ElementNode> = el
            .children
            .iter()
            .filter_map(|c| match c {
                Node::Element(col) if col.tag == UetlTag::Col => Some(col),
                _ => None,
            })
            .collect();

        let flexbox_supported = self.profile.supports("css_flexbox").is_supported();
        let media_queries_supported = self.profile.supports("media_queries").is_supported();

        if stack_on_mobile && media_queries_supported {
            let class = self.next_class("ue-row");
            self.push_style_rule(format!(
                "@media (max-width:600px){{.{class}{{display:block!important;width:100%!important;}} .{class} .ue-col{{display:block!important;width:100%!important;}}}}"
            ));
            return self.render_row_cells(&cols, gap, background, padding, flexbox_supported, Some(&class));
        }

        if stack_on_mobile && !media_queries_supported {
            // Pas de media queries disponibles : on force directement une colonne unique.
            return self.render_stacked_cells(&cols, background, padding);
        }

        self.render_row_cells(&cols, gap, background, padding, flexbox_supported, None)
    }

    fn render_row_cells(
        &self,
        cols: &[&ElementNode],
        gap: Option<String>,
        background: Option<String>,
        padding: Option<String>,
        flexbox_supported: bool,
        class: Option<&str>,
    ) -> String {
        if flexbox_supported {
            let container_class = class.map(|c| format!(" class=\"{c}\"")).unwrap_or_default();
            let container_style = style_attr(&[
                ("display", Some("flex".into())),
                ("gap", gap),
                ("background", background),
                ("padding", padding),
            ]);
            let cells = cols.iter().fold(String::new(), |mut acc, col| {
                let content = self.gen_col(col);
                let _ = write!(acc, "<div class=\"ue-col\" style=\"flex:1;\">{content}</div>");
                acc
            });
            format!("<div{container_class}{container_style}>{cells}</div>")
        } else {
            let table_class = class.map(|c| format!(" class=\"{c}\"")).unwrap_or_default();
            let table_style = style_attr(&[("background", background), ("padding", padding)]);
            let cells = cols.iter().fold(String::new(), |mut acc, col| {
                let content = self.gen_col(col);
                let _ = write!(acc, "<td class=\"ue-col\" valign=\"top\">{content}</td>");
                acc
            });
            format!(
                "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"{table_class}{table_style}><tr>{cells}</tr></table>"
            )
        }
    }

    fn render_stacked_cells(
        &self,
        cols: &[&ElementNode],
        background: Option<String>,
        padding: Option<String>,
    ) -> String {
        let table_style = style_attr(&[("background", background), ("padding", padding)]);
        let rows = cols.iter().fold(String::new(), |mut acc, col| {
            let _ = write!(acc, "<tr><td>{}</td></tr>", self.gen_col(col));
            acc
        });
        format!("<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"{table_style}>{rows}</table>")
    }

    fn gen_col(&self, el: &ElementNode) -> String {
        self.gen_children(&el.children)
    }

    fn gen_heading(&self, el: &ElementNode) -> String {
        let level = attr_str(&el.attrs, "level").unwrap_or_else(|| "1".to_string());
        let color = attr_str(&el.attrs, "color-light").as_deref().map(normalize_color);
        let font_size = attr_str(&el.attrs, "font-size").as_deref().map(css_unit);
        let align = attr_str(&el.attrs, "align");
        let style = style_attr(&[("color", color), ("font-size", font_size), ("text-align", align)]);
        let class_attr = self.dark_mode_attrs(&el.attrs, "color-dark", "color");
        let content = self.gen_children(&el.children);
        format!("<h{level}{class_attr}{style}>{content}</h{level}>")
    }

    fn gen_text(&self, el: &ElementNode) -> String {
        let color = attr_str(&el.attrs, "color-light").as_deref().map(normalize_color);
        let font_size = attr_str(&el.attrs, "font-size").as_deref().map(css_unit);
        let line_height = attr_str(&el.attrs, "line-height");
        let style = style_attr(&[
            ("color", color),
            ("font-size", font_size),
            ("line-height", line_height),
        ]);
        let class_attr = self.dark_mode_attrs(&el.attrs, "color-dark", "color");
        let content = self.gen_children(&el.children);
        format!("<p{class_attr}{style}>{content}</p>")
    }

    fn gen_button(&self, el: &ElementNode) -> String {
        let href = attr_str(&el.attrs, "href").unwrap_or_default();
        let label = self.gen_children(&el.children);
        let accessible_label = attr_str(&el.attrs, "accessible-label");

        // `theme` fournit un preset; `background` et `color` le surchargent.
        // Sans cette surcharge un bouton ne pouvait porter que l'une des trois
        // couleurs codees en dur, ce qui rendait toute charte graphique
        // inapplicable a l'element le plus charge en identite d'un email.
        let (theme_background, theme_color) =
            theme_colors(attr_str(&el.attrs, "theme").as_deref());
        let background = attr_str(&el.attrs, "background")
            .as_deref()
            .map(normalize_color)
            .unwrap_or(theme_background);
        let color = attr_str(&el.attrs, "color")
            .as_deref()
            .map(normalize_color)
            .unwrap_or(theme_color);
        let aria = accessible_label
            .map(|l| format!(" aria-label=\"{}\"", html_escape(&l)))
            .unwrap_or_default();

        if self.profile.quirk("vml_support") {
            format!(
                "<!--[if mso]>\
<v:roundrect xmlns:v=\"urn:schemas-microsoft-com:vml\" href=\"{href}\" style=\"height:44px;v-text-anchor:middle;width:200px;\" arcsize=\"10%\" stroke=\"f\" fillcolor=\"{background}\">\
<center style=\"color:{color};font-family:Arial,sans-serif;font-size:16px;\">{label}</center></v:roundrect>\
<![endif]-->\
<!--[if !mso]><!-->\
<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\"><tr><td align=\"center\" bgcolor=\"{background}\" style=\"border-radius:4px;\">\
<a href=\"{href}\"{aria} style=\"font-size:16px;font-family:Arial,sans-serif;color:{color};text-decoration:none;padding:12px 24px;display:inline-block;\">{label}</a>\
</td></tr></table>\
<!--<![endif]-->"
            )
        } else {
            format!(
                "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\"><tr><td align=\"center\" bgcolor=\"{background}\" style=\"border-radius:4px;\">\
<a href=\"{href}\"{aria} style=\"font-size:16px;font-family:Arial,sans-serif;color:{color};text-decoration:none;padding:12px 24px;display:inline-block;\">{label}</a>\
</td></tr></table>"
            )
        }
    }

    fn gen_image(&self, el: &ElementNode) -> String {
        let src = attr_str(&el.attrs, "src").unwrap_or_default();
        let alt = attr_str(&el.attrs, "alt").unwrap_or_default();
        let width = attr_str(&el.attrs, "width").as_deref().map(css_unit);
        let height = attr_str(&el.attrs, "height").as_deref().map(css_unit);
        let dark_src = attr_str(&el.attrs, "dark-src");
        let style = style_attr(&[("width", width.clone()), ("height", height.clone())]);
        let width_attr = width.map(|w| format!(" width=\"{w}\"")).unwrap_or_default();

        if let Some(dark_src) = dark_src {
            if self.profile.supports("dark_mode_media_query").is_supported() {
                // Deux images superposees, basculees par media query, plutot que
                // <picture> : Gmail supprime purement et simplement cette balise
                // et Outlook l'ignore, de sorte que la variante sombre n'etait
                // jamais utilisee. La bascule par classe est la technique
                // reellement supportee en email, et c'est deja celle qu'utilise
                // dark_media_class_for_attr pour les couleurs.
                let light_class = self.next_class("ue-dark");
                let dark_class = self.next_class("ue-dark");

                self.push_style_rule(format!(
                    "@media (prefers-color-scheme:dark){{\
.{light_class}{{display:none !important;}}\
.{dark_class}{{display:inline-block !important;}}}}"
                ));

                // mso-hide masque la variante sombre dans Outlook, qui ne
                // comprend pas les media queries et afficherait les deux.
                return format!(
                    "<span class=\"{light_class}\">\
<img src=\"{src}\" alt=\"{alt}\"{width_attr}{style} /></span>\
<span class=\"{dark_class}\" style=\"display:none;mso-hide:all;\">\
<img src=\"{dark_src}\" alt=\"{alt}\"{width_attr}{style} /></span>"
                );
            }
        }

        format!("<img src=\"{src}\" alt=\"{alt}\"{width_attr}{style} />")
    }

    fn gen_divider(&self, el: &ElementNode) -> String {
        let color = attr_str(&el.attrs, "color")
            .as_deref().map(normalize_color)
            .unwrap_or_else(|| "#cccccc".to_string());
        let thickness = attr_str(&el.attrs, "thickness")
            .as_deref().map(css_unit)
            .unwrap_or_else(|| "1px".to_string());
        let margin = attr_str(&el.attrs, "margin").as_deref().map(css_unit);
        let style = style_attr(&[
            ("border", Some("0".to_string())),
            ("border-top", Some(format!("{thickness} solid {color}"))),
            ("margin", margin),
        ]);
        format!("<hr{style} />")
    }

    fn gen_spacer(&self, el: &ElementNode) -> String {
        let height = attr_str(&el.attrs, "height").as_deref().map(css_unit).unwrap_or_else(|| "20px".to_string());
        format!(
            "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"><tr><td style=\"height:{height};line-height:{height};font-size:1px;\">&nbsp;</td></tr></table>"
        )
    }

    fn gen_interactive(&self, el: &ElementNode) -> String {
        if let Some(fallback) = attr_str(&el.attrs, "fallback-src") {
            format!("<img src=\"{fallback}\" alt=\"\" />")
        } else {
            self.gen_children(&el.children)
        }
    }

    fn gen_raw(&self, el: &ElementNode) -> String {
        el.children
            .iter()
            .map(|child| match child {
                Node::Text(text) => text.clone(),
                other => self.gen_node(other),
            })
            .collect()
    }
}

fn theme_colors(theme: Option<&str>) -> (String, String) {
    match theme {
        Some("secondary") => ("#6c757d".to_string(), "#ffffff".to_string()),
        Some("danger") => ("#d9534f".to_string(), "#ffffff".to_string()),
        _ => ("#2E5FAC".to_string(), "#ffffff".to_string()),
    }
}

fn attr_str(attrs: &HashMap<String, AttrValue>, name: &str) -> Option<String> {
    attrs.get(name).map(|v| match v {
        AttrValue::String(s) => s.clone(),
        AttrValue::Template(expr) => format!("{{{{{expr}}}}}"),
        AttrValue::Bool(b) => b.to_string(),
    })
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
