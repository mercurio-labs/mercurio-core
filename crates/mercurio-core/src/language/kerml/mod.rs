pub mod parser;

pub use crate::language::modules::KermlLanguageModule;
pub use parser::{compile_text, compile_text_with_context, parse};
