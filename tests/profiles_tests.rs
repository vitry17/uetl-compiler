use uetl_compiler::compiler::{ProfileRegistry, SupportLevel};

#[test]
fn loads_all_seven_profiles() {
    let registry = ProfileRegistry::load();
    let mut ids: Vec<&str> = registry.list_profiles().iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        vec![
            "apple_mail",
            "gmail",
            "outlook_365",
            "outlook_desktop",
            "samsung_mail",
            "thunderbird",
            "yahoo_mail",
        ]
    );
}

#[test]
fn get_profile_finds_known_client_and_rejects_unknown() {
    let registry = ProfileRegistry::load();
    assert!(registry.get_profile("gmail").is_some());
    assert!(registry.get_profile("does-not-exist").is_none());
}

#[test]
fn supports_resolves_full_partial_and_none() {
    let registry = ProfileRegistry::load();

    let thunderbird = registry.get_profile("thunderbird").unwrap();
    assert_eq!(thunderbird.supports("css_flexbox"), SupportLevel::Full);

    let gmail = registry.get_profile("gmail").unwrap();
    assert_eq!(gmail.supports("background_image"), SupportLevel::Partial);

    let outlook_desktop = registry.get_profile("outlook_desktop").unwrap();
    assert_eq!(outlook_desktop.supports("css_grid"), SupportLevel::None);
}

#[test]
fn quirk_reports_client_specific_rendering_constraints() {
    let registry = ProfileRegistry::load();

    let outlook_desktop = registry.get_profile("outlook_desktop").unwrap();
    assert!(outlook_desktop.quirk("vml_support"));

    let gmail = registry.get_profile("gmail").unwrap();
    assert!(!gmail.quirk("vml_support"));
}
