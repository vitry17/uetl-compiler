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
        for raw in ALL_PROFILES {
            let profile: Profile =
                serde_json::from_str(raw).expect("bundled profile JSON must be valid");
            profiles.insert(profile.id.clone(), profile);
        }
        Self { profiles }
    }

    pub fn get_profile(&self, client: &str) -> Option<&Profile> {
        self.profiles.get(client)
    }

    pub fn list_profiles(&self) -> Vec<&Profile> {
        self.profiles.values().collect()
    }

    /// Instance partagée, chargée une seule fois au premier appel (pas de reparsing par requête).
    pub fn shared() -> &'static ProfileRegistry {
        static REGISTRY: OnceLock<ProfileRegistry> = OnceLock::new();
        REGISTRY.get_or_init(ProfileRegistry::load)
    }
}
