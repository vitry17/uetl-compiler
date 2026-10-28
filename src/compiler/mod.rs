pub mod css_inliner;
pub mod html_gen;
pub mod lint;
pub mod profiles;

pub use html_gen::HtmlGenerator;
pub use lint::collect_warnings;
pub use profiles::{Profile, ProfileRegistry, SupportLevel};
