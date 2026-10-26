use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Niveau de support d'une fonctionnalité CSS par un client mail.
/// Les profils JSON expriment ça en `true`/`false`/`"partial"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportLevel {
    Full,
    Partial,
    None,
}

impl SupportLevel {
    pub fn is_supported(&self) -> bool {
        matches!(self, SupportLevel::Full)
    }
}

/// Clés `supports`/`quirks` reconnues, à travers les 7 profils embarqués —
/// **volontairement écrite en dur**, plutôt que dérivée des fichiers JSON
/// eux-mêmes : dériver la liste des fichiers qu'elle est censée vérifier la
/// rendrait tautologique (une faute de frappe deviendrait juste une nouvelle
/// clé "connue"). Chaque profil ne déclare pas forcément toutes ces clés —
/// `quirks."dark_mode_data_attributes"` par exemple n'existe que dans
/// `yahoo_mail.json`, une clé absente valant `false`/`None` par défaut
/// (voir `supports`/`quirk` ci-dessous) — mais aucune clé hors de cette
/// liste n'est acceptée : une faute de frappe (`"vml_suport"`) qui
/// résoudrait sinon silencieusement à `false` est rejetée au chargement.
const KNOWN_SUPPORTS_KEYS: &[&str] = &[
    "css_flexbox",
    "css_grid",
    "css_variables",
    "css_animations",
    "border_radius",
    "background_image",
    "media_queries",
    "dark_mode_media_query",
    "position_absolute",
    "margin_auto",
    "padding_shorthand",
    "max_width",
    "min_width",
];

const KNOWN_QUIRKS_KEYS: &[&str] = &[
    "table_layout_required",
    "inline_styles_only",
    "strips_style_tags",
    "vml_support",
    "gmail_class_prefix",
    "dark_mode_data_attributes",
];

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub version: String,
    pub supports: Value,
    pub quirks: Value,
    pub button_strategy: String,
    pub layout_strategy: String,
    pub image_strategy: String,
}

impl Profile {
    /// Valide `supports`/`quirks` contre les clés et types attendus.
    /// Renvoie la liste de tous les problèmes trouvés (pas seulement le
    /// premier) pour qu'une seule relecture du message d'erreur suffise à
    /// tout corriger.
    fn validate_schema(&self) -> Vec<String> {
        let mut problems = Vec::new();
        check_object(
            &self.id,
            "supports",
            &self.supports,
            KNOWN_SUPPORTS_KEYS,
            &mut problems,
        );
        check_object(
            &self.id,
            "quirks",
            &self.quirks,
            KNOWN_QUIRKS_KEYS,
            &mut problems,
        );
        problems
    }

    /// Niveau de support d'une fonctionnalité (ex: "css_flexbox", "dark_mode_media_query").
    /// Utilisé par le générateur HTML pour choisir une stratégie de rendu par composant.
    pub fn supports(&self, feature: &str) -> SupportLevel {
        match self.supports.get(feature) {
            Some(Value::Bool(true)) => SupportLevel::Full,
            Some(Value::String(s)) if s == "partial" || s == "limited" => SupportLevel::Partial,
            _ => SupportLevel::None,
        }
    }

    /// Particularité booléenne du client (ex: "vml_support", "table_layout_required").
    pub fn quirk(&self, name: &str) -> bool {
        matches!(self.quirks.get(name), Some(Value::Bool(true)))
    }
}

fn check_object(
    profile_id: &str,
    field: &str,
    value: &Value,
    known_keys: &[&str],
    problems: &mut Vec<String>,
) {
    let Value::Object(map) = value else {
        problems.push(format!("{profile_id}: `{field}` must be a JSON object"));
        return;
    };
    for (key, val) in map {
        if !known_keys.contains(&key.as_str()) {
            problems.push(format!(
                "{profile_id}: unknown {field} key `{key}` (typo? see KNOWN_{}_KEYS in profiles.rs)",
                field.to_uppercase()
            ));
            continue;
        }
        let valid_value = match field {
            // `supports` autorise aussi "partial"/"limited" (voir `Profile::supports`).
            "supports" => {
                matches!(val, Value::Bool(_))
                    || matches!(val, Value::String(s) if s == "partial" || s == "limited")
            }
            _ => matches!(val, Value::Bool(_)),
        };
        if !valid_value {
            problems.push(format!(
                "{profile_id}: {field}.{key} has an unexpected value {val} (expected a bool{})",
                if field == "supports" {
                    " or \"partial\"/\"limited\""
                } else {
                    ""
                }
            ));
        }
    }
}

const GMAIL: &str = include_str!("../profiles/gmail.json");
const OUTLOOK_DESKTOP: &str = include_str!("../profiles/outlook_desktop.json");
const OUTLOOK_365: &str = include_str!("../profiles/outlook_365.json");
const APPLE_MAIL: &str = include_str!("../profiles/apple_mail.json");
const YAHOO_MAIL: &str = include_str!("../profiles/yahoo_mail.json");
const THUNDERBIRD: &str = include_str!("../profiles/thunderbird.json");
const SAMSUNG_MAIL: &str = include_str!("../profiles/samsung_mail.json");

const ALL_PROFILES: [&str; 7] = [
    GMAIL,
    OUTLOOK_DESKTOP,
    OUTLOOK_365,
    APPLE_MAIL,
    YAHOO_MAIL,
    THUNDERBIRD,
    SAMSUNG_MAIL,
];

pub struct ProfileRegistry {
    profiles: HashMap<String, Profile>,
}

impl ProfileRegistry {
    /// Charge les profils embarqués dans le binaire (aucune lecture disque à la requête).
    pub fn load() -> Self {
        let mut profiles = HashMap::new();
        let mut schema_problems = Vec::new();
        for raw in ALL_PROFILES {
            let profile: Profile =
                serde_json::from_str(raw).expect("bundled profile JSON must be valid");
            schema_problems.extend(profile.validate_schema());
            profiles.insert(profile.id.clone(), profile);
        }
        // Les profils sont embarqués au build, pas une entrée utilisateur :
        // une clé de schéma invalide est un bug du dépôt, pas une erreur
        // récupérable à la requête — paniquer au chargement (donc au
        // démarrage, une fois `/health` corrigé pour forcer ce chargement)
        // le rend impossible à manquer, au lieu de silencieusement
        // désactiver la fonctionnalité concernée en production.
        assert!(
            schema_problems.is_empty(),
            "invalid profile schema:\n{}",
            schema_problems.join("\n")
        );
        Self { profiles }
    }

    pub fn get_profile(&self, client: &str) -> Option<&Profile> {
        self.profiles.get(client)
    }

    /// Trié par id : `HashMap::values()` n'a aucun ordre garanti d'un
    /// process à l'autre, ce qui rendait `/compile/all` non déterministe
    /// (voir `CompileAllResponse::results`) même après son passage à `IndexMap`.
    pub fn list_profiles(&self) -> Vec<&Profile> {
        let mut profiles: Vec<&Profile> = self.profiles.values().collect();
        profiles.sort_by(|a, b| a.id.cmp(&b.id));
        profiles
    }

    /// Instance partagée, chargée une seule fois au premier appel (pas de reparsing par requête).
    pub fn shared() -> &'static ProfileRegistry {
        static REGISTRY: OnceLock<ProfileRegistry> = OnceLock::new();
        REGISTRY.get_or_init(ProfileRegistry::load)
    }
}

#[cfg(test)]
mod schema_tests {
    use super::*;

    #[test]
    fn all_seven_bundled_profiles_pass_schema_validation() {
        // `ProfileRegistry::load()` paniquerait déjà si ce n'était pas le
        // cas — ce test documente explicitement l'intention et échoue avec
        // un message clair plutôt qu'un panic de test générique.
        for raw in ALL_PROFILES {
            let profile: Profile = serde_json::from_str(raw).unwrap();
            let problems = profile.validate_schema();
            assert!(problems.is_empty(), "{}: {problems:?}", profile.id);
        }
    }

    #[test]
    fn rejects_an_unknown_supports_key_as_a_likely_typo() {
        let profile: Profile = serde_json::from_str(
            r#"{
                "id": "test", "name": "Test", "version": "1",
                "supports": { "css_flexbox": true, "css_flexbocks": true },
                "quirks": {},
                "button_strategy": "table-cell", "layout_strategy": "table", "image_strategy": "standard"
            }"#,
        )
        .unwrap();
        let problems = profile.validate_schema();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("css_flexbocks"));
    }

    #[test]
    fn rejects_an_unknown_quirks_key() {
        let profile: Profile = serde_json::from_str(
            r#"{
                "id": "test", "name": "Test", "version": "1",
                "supports": {},
                "quirks": { "vml_suport": true },
                "button_strategy": "table-cell", "layout_strategy": "table", "image_strategy": "standard"
            }"#,
        )
        .unwrap();
        let problems = profile.validate_schema();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("vml_suport"));
    }

    #[test]
    fn rejects_a_non_boolean_quirk_value() {
        let profile: Profile = serde_json::from_str(
            r#"{
                "id": "test", "name": "Test", "version": "1",
                "supports": {},
                "quirks": { "vml_support": "yes" },
                "button_strategy": "table-cell", "layout_strategy": "table", "image_strategy": "standard"
            }"#,
        )
        .unwrap();
        let problems = profile.validate_schema();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("vml_support"));
    }

    #[test]
    fn accepts_partial_and_limited_as_supports_values() {
        let profile: Profile = serde_json::from_str(
            r#"{
                "id": "test", "name": "Test", "version": "1",
                "supports": { "background_image": "partial", "css_grid": "limited" },
                "quirks": {},
                "button_strategy": "table-cell", "layout_strategy": "table", "image_strategy": "standard"
            }"#,
        )
        .unwrap();
        assert!(profile.validate_schema().is_empty());
    }
}
