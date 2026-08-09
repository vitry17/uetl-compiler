use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Write;

use super::css_inliner::{css_unit, normalize_color, style_attr};
use super::profiles::Profile;
use crate::parser::ast::{AttrValue, DarkModeOption, DocumentNode, ElementNode, Node, UetlTag};

/// Pile de polices par defaut.
///
/// Le compilateur n'emettait aucune `font-family` : chaque client appliquait
/// donc la sienne, souvent une serif dans Outlook, ce qui ne ressemblait a
/// aucune charte graphique moderne. Cette pile ne contient que des polices
/// reellement installees sur les postes clients — une webfont ne se charge
/// pas dans la majorite des clients mail.
const DEFAULT_FONT_STACK: &str =
    "-apple-system, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif";

pub struct HtmlGenerator<'a> {
    profile: &'a Profile,
    /// Police du document. Emise sur chaque bloc textuel plutot que sur le
    /// seul `<body>` : le moteur Word d'Outlook n'herite pas la police dans
    /// les tableaux, et toute la mise en page en est faite.
    font_family: String,
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
            font_family: ast
                .font_family
                .clone()
                .unwrap_or_else(|| DEFAULT_FONT_STACK.to_string()),
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

    /// Declarations CSS d'une « boite » stylable — `ue-row` et `ue-col`.
    ///
    /// Sans elles une colonne ne portait aucun style : `gen_col` se contentait
    /// de rendre ses enfants. Les elements les plus courants d'un email reel —
    /// tuiles d'arguments, encadres colores, blocs partenaires — sont pourtant
    /// des colonnes a fond, marge interieure et angles arrondis. Ils etaient
    /// donc inexprimables, quelle que soit l'habilete de l'auteur.
    ///
    /// `border-radius` est ignore par le moteur Word d'Outlook : les angles y
    /// restent droits. C'est une degradation acceptable, le fond et la marge
    /// etant eux honores.
    fn box_style_decls(&self, el: &ElementNode) -> Vec<(&'static str, Option<String>)> {
        vec![
            (
                "background",
                attr_themed(&el.attrs, "background").as_deref().map(normalize_color),
            ),
            ("padding", attr_str(&el.attrs, "padding").as_deref().map(css_unit)),
            ("border", attr_str(&el.attrs, "border")),
            (
                "border-radius",
                attr_str(&el.attrs, "border-radius").as_deref().map(css_unit),
            ),
            ("text-align", attr_str(&el.attrs, "align")),
        ]
    }

    /// Attributs non-CSS d'une boite : classes (dont celle du mode sombre),
    /// `data-ogsb` pour Yahoo/AOL, et `bgcolor` sur les cellules de tableau.
    ///
    /// `bgcolor` double la declaration CSS parce que le moteur Word d'Outlook
    /// l'honore de facon bien plus fiable qu'un `background`. Il n'a en
    /// revanche aucun sens sur un `<div>`, d'ou `with_bgcolor`.
    ///
    /// A n'appeler qu'une fois par element : la classe de mode sombre est
    /// generee au vol et enregistre une regle CSS au passage.
    fn box_marker_attrs(
        &self,
        el: &ElementNode,
        base_class: Option<&str>,
        with_bgcolor: bool,
    ) -> String {
        let mut classes: Vec<String> = base_class.into_iter().map(String::from).collect();

        if let Some(class) = self.dark_media_class_for_attr(&el.attrs, "background-dark", "background")
        {
            classes.push(class);
        }

        let class_attr = if classes.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", classes.join(" "))
        };

        let bgcolor = if with_bgcolor {
            attr_themed(&el.attrs, "background")
                .as_deref()
                .map(normalize_color)
                .map(|color| format!(" bgcolor=\"{color}\""))
                .unwrap_or_default()
        } else {
            String::new()
        };

        let data_attr = self
            .dark_data_attr_for(&el.attrs, "background-dark", "background")
            .map(|attr| format!(" {attr}"))
            .unwrap_or_default();

        format!("{class_attr}{bgcolor}{data_attr}")
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
            // <strong> et <em> plutot que <b>/<i> : meme rendu partout, y
            // compris dans le moteur Word d'Outlook, et le sens est porte
            // pour les lecteurs d'ecran.
            UetlTag::Bold => format!("<strong>{}</strong>", self.gen_children(&el.children)),
            UetlTag::Italic => format!("<em>{}</em>", self.gen_children(&el.children)),
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
        let background = attr_themed(&el.attrs, "background").as_deref().map(normalize_color);

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
            return self.render_row_cells(el, &cols, flexbox_supported, Some(&class));
        }

        if stack_on_mobile && !media_queries_supported {
            // Pas de media queries disponibles : on force directement une colonne unique.
            return self.render_stacked_cells(el, &cols);
        }

        self.render_row_cells(el, &cols, flexbox_supported, None)
    }

    fn render_row_cells(
        &self,
        row: &ElementNode,
        cols: &[&ElementNode],
        flexbox_supported: bool,
        class: Option<&str>,
    ) -> String {
        let gap = attr_str(&row.attrs, "gap").as_deref().map(css_unit);

        if flexbox_supported {
            let container_attrs = self.box_marker_attrs(row, class, false);
            let mut decls = self.box_style_decls(row);
            decls.push(("display", Some("flex".into())));
            decls.push(("gap", gap));
            let container_style = style_attr(&decls);

            let cells = cols.iter().fold(String::new(), |mut acc, col| {
                let col_attrs = self.box_marker_attrs(col, Some("ue-col"), false);
                let mut col_decls = self.box_style_decls(col);
                col_decls.push(("flex", Some("1".into())));
                let col_style = style_attr(&col_decls);
                let content = self.gen_col(col);
                let _ = write!(acc, "<div{col_attrs}{col_style}>{content}</div>");
                acc
            });

            format!("<div{container_attrs}{container_style}>{cells}</div>")
        } else {
            let table_attrs = self.box_marker_attrs(row, class, true);
            let table_style = style_attr(&self.box_style_decls(row));

            // `gap` est une propriete flexbox : une cellule de tableau ne la
            // connait pas, et l'attribut disparaissait purement et simplement
            // pour Gmail et Outlook — c'est-a-dire la majorite des lecteurs.
            // Les cartes se touchaient. On intercale donc de vraies cellules
            // d'espacement, seule technique fiable en email.
            let spacer = gap.as_deref().map(|width| self.gap_cell(width, class));

            let cells = cols.iter().enumerate().fold(String::new(), |mut acc, (index, col)| {
                if index > 0 {
                    if let Some(spacer) = spacer.as_deref() {
                        acc.push_str(spacer);
                    }
                }
                let _ = write!(acc, "{}", self.render_col_cell(col));
                acc
            });

            format!(
                "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"{table_attrs}{table_style}><tr>{cells}</tr></table>"
            )
        }
    }

    fn render_stacked_cells(&self, row: &ElementNode, cols: &[&ElementNode]) -> String {
        let table_attrs = self.box_marker_attrs(row, None, true);
        let table_style = style_attr(&self.box_style_decls(row));

        let rows = cols.iter().fold(String::new(), |mut acc, col| {
            let _ = write!(acc, "<tr>{}</tr>", self.render_col_cell(col));
            acc
        });

        format!(
            "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"{table_attrs}{table_style}>{rows}</table>"
        )
    }

    /// Cellule d'espacement intercalee entre deux colonnes.
    ///
    /// `font-size:0` et `line-height:0` empechent le `&nbsp;` d'imposer une
    /// hauteur minimale : sans lui, certains clients effondrent une cellule
    /// vide et l'espacement disparait quand meme.
    ///
    /// Quand la ligne s'empile sur mobile, l'espaceur ne disparait pas : il
    /// devient une bande de la meme hauteur entre deux cartes. Le masquer
    /// collait les cartes les unes aux autres des que la fenetre etait
    /// etroite — c'est-a-dire dans la moitie des lectures.
    fn gap_cell(&self, width: &str, stack_class: Option<&str>) -> String {
        let class_attr = match stack_class {
            Some(row_class) if self.profile.supports("media_queries").is_supported() => {
                let class = self.next_class("ue-gap");
                self.push_style_rule(format!(
                    "@media (max-width:600px){{.{row_class} .{class}{{display:block!important;width:100%!important;height:{width}!important;}}}}"
                ));
                format!(" class=\"{class}\"")
            }
            _ => String::new(),
        };

        // L'attribut width d'une cellule attend un entier nu, comme celui
        // d'une image : `width="12px"` serait invalide et ignore, et Outlook
        // laisserait la cellule s'etirer.
        let width_attr = pixel_count(width)
            .map(|px| format!(" width=\"{px}\""))
            .unwrap_or_default();

        format!(
            "<td{class_attr}{width_attr} style=\"width:{width};font-size:0;line-height:0;\">&nbsp;</td>"
        )
    }

    /// Une colonne rendue en cellule de tableau, avec ses propres styles.
    ///
    /// Les styles sont poses sur le `<td>` plutot que dans un element
    /// supplementaire : Outlook honore `bgcolor` et `padding` sur une cellule,
    /// et chaque niveau d'imbrication en plus est une occasion de divergence
    /// entre clients.
    fn render_col_cell(&self, col: &ElementNode) -> String {
        let attrs = self.box_marker_attrs(col, Some("ue-col"), true);
        let style = style_attr(&self.box_style_decls(col));
        let content = self.gen_col(col);

        format!("<td{attrs} valign=\"top\"{style}>{content}</td>")
    }

    fn gen_col(&self, el: &ElementNode) -> String {
        self.gen_children(&el.children)
    }

    fn gen_heading(&self, el: &ElementNode) -> String {
        let level = attr_str(&el.attrs, "level").unwrap_or_else(|| "1".to_string());
        let color = attr_themed(&el.attrs, "color").as_deref().map(normalize_color);
        let font_size = attr_str(&el.attrs, "font-size").as_deref().map(css_unit);
        let align = attr_str(&el.attrs, "align");
        let style = style_attr(&[
            ("font-family", Some(self.font_family.clone())),
            ("color", color),
            ("font-size", font_size),
            ("text-align", align),
        ]);
        let class_attr = self.dark_mode_attrs(&el.attrs, "color-dark", "color");
        let content = self.gen_children(&el.children);
        format!("<h{level}{class_attr}{style}>{content}</h{level}>")
    }

    fn gen_text(&self, el: &ElementNode) -> String {
        let color = attr_themed(&el.attrs, "color").as_deref().map(normalize_color);
        let font_size = attr_str(&el.attrs, "font-size").as_deref().map(css_unit);
        let line_height = attr_str(&el.attrs, "line-height");
        let style = style_attr(&[
            ("font-family", Some(self.font_family.clone())),
            ("color", color),
            ("font-size", font_size),
            ("line-height", line_height),
            ("text-align", attr_str(&el.attrs, "align")),
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

        // Le rayon etait ecrit en dur a 4px : aucun bouton en pilule n'etait
        // possible, alors que c'est la forme la plus courante des chartes
        // actuelles.
        let radius = attr_str(&el.attrs, "border-radius")
            .as_deref()
            .map(css_unit)
            .unwrap_or_else(|| "4px".to_string());

        let font = &self.font_family;

        // `align` sur la table plutot qu'un text-align herite : un `<table>`
        // est de niveau bloc, `text-align:center` sur son parent ne le centre
        // donc pas. L'attribut HTML align, lui, est honore par tous les
        // clients, Outlook compris.
        let align = attr_str(&el.attrs, "align")
            .map(|a| format!(" align=\"{a}\""))
            .unwrap_or_default();

        if self.profile.quirk("vml_support") {
            // VML ne connait pas border-radius : il exprime l'arrondi en
            // pourcentage de la moitie du plus petit cote. Le bouton faisant
            // 44px de haut, 50 % correspond a un rayon de 22px — la pilule.
            let arcsize = vml_arcsize(&radius);

            format!(
                "<!--[if mso]>\
<v:roundrect xmlns:v=\"urn:schemas-microsoft-com:vml\" href=\"{href}\" style=\"height:44px;v-text-anchor:middle;width:200px;\" arcsize=\"{arcsize}%\" stroke=\"f\" fillcolor=\"{background}\">\
<center style=\"color:{color};font-family:{font};font-size:16px;\">{label}</center></v:roundrect>\
<![endif]-->\
<!--[if !mso]><!-->\
<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\"{align}><tr><td align=\"center\" bgcolor=\"{background}\" style=\"border-radius:{radius};\">\
<a href=\"{href}\"{aria} style=\"font-size:16px;font-family:{font};color:{color};text-decoration:none;padding:12px 24px;display:inline-block;\">{label}</a>\
</td></tr></table>\
<!--<![endif]-->"
            )
        } else {
            format!(
                "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\"{align}><tr><td align=\"center\" bgcolor=\"{background}\" style=\"border-radius:{radius};\">\
<a href=\"{href}\"{aria} style=\"font-size:16px;font-family:{font};color:{color};text-decoration:none;padding:12px 24px;display:inline-block;\">{label}</a>\
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
        // Une image est en ligne : `align` sur la colonne parente la centre
        // deja via text-align. Seul l'arrondi manquait, tres courant sur les
        // visuels d'en-tete. Outlook l'ignore, l'image y reste a angles droits.
        let style = style_attr(&[
            ("width", width.clone()),
            ("height", height.clone()),
            (
                "border-radius",
                attr_str(&el.attrs, "border-radius").as_deref().map(css_unit),
            ),
        ]);
        // L'attribut HTML width attend un entier NU : `width="160px"` est
        // invalide et se fait ignorer, ce qui laisse Outlook afficher l'image
        // a sa taille native — souvent deux fois trop grande. La CSS garde
        // l'unite, l'attribut ne prend que le nombre.
        let width_attr = width
            .as_deref()
            .and_then(pixel_count)
            .map(|w| format!(" width=\"{w}\""))
            .unwrap_or_default();

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

/// Nombre de pixels d'une dimension CSS, pour les attributs HTML qui exigent
/// un entier nu (`width` sur `<img>`). Retourne None pour une valeur relative
/// (`100%`, `auto`) : mieux vaut omettre l'attribut que le remplir de travers.
fn pixel_count(value: &str) -> Option<u32> {
    value.strip_suffix("px")?.trim().parse::<f64>().ok().map(|v| v.round() as u32)
}

/// Traduit un `border-radius` CSS en `arcsize` VML, seul arrondi qu'Outlook
/// comprenne. VML l'exprime en pourcentage de la moitie du plus petit cote ;
/// le bouton faisant 44px de haut, 50 % vaut un rayon de 22px, et tout rayon
/// superieur est ramene a cette valeur — au-dela, la forme ne change plus.
fn vml_arcsize(radius: &str) -> u32 {
    let px: f64 = radius
        .trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.')
        .parse()
        .unwrap_or(4.0);

    ((px / 22.0) * 50.0).round().clamp(0.0, 50.0) as u32
}

/// Lit `nom-light` en priorite, puis `nom`. Les deux ecritures coexistaient
/// selon les balises — `ue-layout` n'acceptait que `background-light`, `ue-row`
/// que `background` — ce qui rendait le langage imprevisible.
fn attr_themed(attrs: &HashMap<String, AttrValue>, name: &str) -> Option<String> {
    attr_str(attrs, &format!("{name}-light")).or_else(|| attr_str(attrs, name))
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
