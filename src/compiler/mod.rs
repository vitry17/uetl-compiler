pub mod css_inliner;
pub mod html_gen;
pub mod lint;
pub mod plain_text;
pub mod profiles;

pub use html_gen::HtmlGenerator;
pub use lint::{collect_structure_warnings, collect_warnings};
pub use plain_text::to_plain_text;
pub use profiles::{Profile, ProfileRegistry, SupportLevel};
