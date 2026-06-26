use criterion::{black_box, criterion_group, criterion_main, Criterion};
use uetl_compiler::compiler::{HtmlGenerator, ProfileRegistry};
use uetl_compiler::parser::Parser;

/// Email représentatif : logo, deux colonnes responsive, bouton, image dark-mode.
const DOCUMENT: &str = r##"<ue-email lang="fr" dark-mode="auto">
<ue-layout max-width="600px">
  <ue-row>
    <ue-col>
      <ue-image src="logo.png" alt="Logo" width="150" />
    </ue-col>
  </ue-row>
  <ue-row stack-on="mobile">
    <ue-col>
      <ue-heading level="1" color-light="#111111">Bonjour {{prenom}}</ue-heading>
      <ue-text>Decouvrez nos offres adaptees a votre profil.</ue-text>
      <ue-button href="{{cta_url}}" theme="primary">Voir mes offres</ue-button>
    </ue-col>
    <ue-col>
      <ue-image src="produit.jpg" alt="Produit" dark-src="produit-dark.jpg" />
      <ue-divider />
      <ue-spacer height="20px" />
    </ue-col>
  </ue-row>
</ue-layout>
</ue-email>"##;

fn bench_parse(c: &mut Criterion) {
    c.bench_function("parse_document", |b| {
        b.iter(|| Parser::parse_document(black_box(DOCUMENT)).unwrap());
    });
}

fn bench_compile_one_profile(c: &mut Criterion) {
    let registry = ProfileRegistry::load();
    let profile = registry.get_profile("gmail").unwrap();
    let doc = Parser::parse_document(DOCUMENT).unwrap();

    c.bench_function("generate_html_gmail", |b| {
        b.iter(|| HtmlGenerator::generate(black_box(&doc), black_box(profile)));
    });
}

fn bench_compile_all_profiles(c: &mut Criterion) {
    let registry = ProfileRegistry::load();
    let doc = Parser::parse_document(DOCUMENT).unwrap();

    c.bench_function("compile_all_seven_profiles", |b| {
        b.iter(|| {
            for profile in registry.list_profiles() {
                HtmlGenerator::generate(black_box(&doc), black_box(profile));
            }
        });
    });
}

criterion_group!(
    benches,
    bench_parse,
    bench_compile_one_profile,
    bench_compile_all_profiles
);
criterion_main!(benches);
