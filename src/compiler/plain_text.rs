//! Version texte brut d'un email, produite depuis l'arbre UETL.
//!
//! Pourquoi depuis l'arbre et pas en nettoyant le HTML : l'arbre sait ce
//! qu'est un titre, un paragraphe ou un bouton. Le HTML compilé, lui, est
//! un empilement de tableaux de mise en page où cette information est
//! perdue. Un nettoyage de HTML produit des fragments dans le désordre,
//! des espaces parasites et des URL sans contexte.
//!
//! Cette version sert trois lecteurs : les clients qui n'affichent pas le
//! HTML, les filtres anti-spam (un mail HTML sans partie texte est un signal
//! négatif) et les assistants IA, qui préfèrent une copie propre et linéaire.
//!
//! Règles :
//! - un bloc (titre, paragraphe, bouton, séparateur) par paragraphe de texte ;
//! - un bouton devient `Libellé : URL`, pour garder le lien et son sens ;
//! - une image n'apparaît que par son texte alternatif, s'il en a un ;
//! - le préheader (texte caché) n'est jamais repris : il n'est pas visible
//!   dans le corps, il ne doit pas l'être ici non plus ;
//! - les expressions `{{ ... }}` et blocs `{% ... %}` sont conservés tels
//!   quels, pour être personnalisés à l'envoi comme le HTML.

use crate::parser::ast::{AttrValue, DocumentNode, ElementNode, Node, UetlTag};

/// Produit la version texte brut d'un document UETL.
pub fn to_plain_text(doc: &DocumentNode) -> String {
    let mut blocks = Vec::new();
    collect_blocks(&doc.children, &mut blocks);
    blocks
        .into_iter()
        .map(|block| block.trim().to_string())
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn collect_blocks(nodes: &[Node], blocks: &mut Vec<String>) {
    for node in nodes {
        match node {
            Node::Element(el) => collect_element(el, blocks),
            // Du texte posé directement dans un conteneur : on le garde
            // plutôt que de perdre une information visible.
            Node::Text(_) | Node::Template(_) => {
                let text = inline_text(std::slice::from_ref(node));
                if !text.is_empty() {
                    blocks.push(text);
                }
            }
            Node::Document(doc) => collect_blocks(&doc.children, blocks),
        }
    }
}

fn collect_element(el: &ElementNode, blocks: &mut Vec<String>) {
    match el.tag {
        UetlTag::Heading | UetlTag::Text | UetlTag::Bold | UetlTag::Italic => {
            blocks.push(inline_text(&el.children));
        }
        UetlTag::Button => {
            let label = inline_text(&el.children);
            match attr(el, "href") {
                Some(href) if !label.is_empty() => blocks.push(format!("{label} : {href}")),
                Some(href) => blocks.push(href),
                None => blocks.push(label),
            }
        }
        UetlTag::Image => {
            if let Some(alt) = attr(el, "alt").filter(|alt| !alt.trim().is_empty()) {
                blocks.push(format!("[{}]", alt.trim()));
            }
        }
        UetlTag::Divider => blocks.push("———".to_string()),
        UetlTag::Spacer => {}
        UetlTag::Raw => blocks.push(strip_html(&raw_source(&el.children))),
        UetlTag::Email
        | UetlTag::Layout
        | UetlTag::Row
        | UetlTag::Col
        | UetlTag::Hero
        | UetlTag::Interactive => collect_blocks(&el.children, blocks),
    }
}

/// Texte d'un bloc : contenu en ligne (gras, italique, variables) aplati,
/// espaces normalisés, entités HTML courantes décodées.
fn inline_text(nodes: &[Node]) -> String {
    let mut out = String::new();
    push_inline(nodes, &mut out);
    collapse_whitespace(&decode_entities(&out))
}

fn push_inline(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(text) => out.push_str(text),
            Node::Template(expr) => {
                out.push_str("{{");
                out.push_str(expr);
                out.push_str("}}");
            }
            Node::Element(el) => match el.tag {
                UetlTag::Raw => out.push_str(&strip_html(&raw_source(&el.children))),
                UetlTag::Image => {}
                _ => push_inline(&el.children, out),
            },
            Node::Document(_) => {}
        }
    }
}

fn raw_source(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            Node::Text(text) => out.push_str(text),
            Node::Template(expr) => {
                out.push_str("{{");
                out.push_str(expr);
                out.push_str("}}");
            }
            Node::Element(el) => out.push_str(&raw_source(&el.children)),
            Node::Document(_) => {}
        }
    }
    out
}

fn attr(el: &ElementNode, name: &str) -> Option<String> {
    el.attrs.get(name).map(|value| match value {
        AttrValue::String(s) => s.clone(),
        AttrValue::Template(expr) => format!("{{{{{expr}}}}}"),
        AttrValue::Bool(b) => b.to_string(),
    })
}

/// Retire les balises d'un fragment `ue-raw`. Volontairement simple : le
/// contenu brut est rare, et un texte un peu imparfait vaut mieux qu'un
/// fragment HTML dans une partie texte. Le contenu des balises `<style>`,
/// `<script>` et des commentaires est ignoré, pas recopié.
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    while i < html.len() {
        let rest = &lower[i..];
        if rest.starts_with("<!--") {
            i += rest.find("-->").map_or(rest.len(), |end| end + 3);
        } else if rest.starts_with("<style") || rest.starts_with("<script") {
            let closing = if rest.starts_with("<style") {
                "</style>"
            } else {
                "</script>"
            };
            i += rest
                .find(closing)
                .map_or(rest.len(), |end| end + closing.len());
        } else if rest.starts_with('<') {
            let tag_end = rest.find('>').map_or(rest.len(), |end| end + 1);
            let tag = &rest[..tag_end];
            if tag.starts_with("<br")
                || tag.starts_with("<p")
                || tag.starts_with("</p")
                || tag.starts_with("<div")
                || tag.starts_with("</div")
            {
                out.push(' ');
            }
            i += tag_end;
        } else {
            let ch = html[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    collapse_whitespace(&decode_entities(&out))
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Espaces normalisés, et suppression des caractères invisibles (espaces
/// de largeur nulle et apparentés) : ils n'ont aucun sens en texte brut et
/// passent pour une manipulation auprès des filtres.
fn collapse_whitespace(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' | '\u{00AD}'
            )
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_comments_and_styles_from_raw_html() {
        let raw = "<style>p{color:red}</style><!-- note --><p>Bonjour&nbsp;<b>Ana</b></p><br>Fin";
        assert_eq!(strip_html(raw), "Bonjour Ana Fin");
    }

    #[test]
    fn removes_invisible_characters() {
        assert_eq!(collapse_whitespace("a\u{200B}b \u{FEFF} c"), "ab c");
    }
}
