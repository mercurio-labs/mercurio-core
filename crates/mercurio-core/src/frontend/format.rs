use std::path::Path;

use crate::language::SourceLanguage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    UnsupportedLanguage,
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedLanguage => write!(f, "formatting is only supported for .sysml files"),
        }
    }
}

impl std::error::Error for FormatError {}

pub fn format_text(input: &str, language: SourceLanguage) -> Result<String, FormatError> {
    match language {
        SourceLanguage::Sysml => Ok(format_sysml_text(input)),
        SourceLanguage::Kerml => Err(FormatError::UnsupportedLanguage),
    }
}

pub fn format_path_text(path: &Path, input: &str) -> Result<String, FormatError> {
    let language = SourceLanguage::from_path(path).ok_or(FormatError::UnsupportedLanguage)?;
    format_text(input, language)
}

pub fn format_sysml_text(input: &str) -> String {
    let mut output = String::new();
    let mut indent = 0usize;
    let mut previous_blank = false;

    for raw_line in input.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if !previous_blank && !output.is_empty() {
                output.push('\n');
                previous_blank = true;
            }
            continue;
        }

        let leading_closes = trimmed.chars().take_while(|ch| *ch == '}').count();
        let line_indent = indent.saturating_sub(leading_closes);
        output.push_str(&"  ".repeat(line_indent));
        output.push_str(&normalize_inline_spacing(trimmed));
        output.push('\n');

        let (opens, closes) = brace_delta(trimmed);
        indent = indent.saturating_add(opens).saturating_sub(closes);
        previous_blank = false;
    }

    if output.is_empty() {
        return String::new();
    }
    output
}

fn brace_delta(line: &str) -> (usize, usize) {
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for ch in line.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => opens += 1,
            '}' => closes += 1,
            _ => {}
        }
    }

    (opens, closes)
}

fn normalize_inline_spacing(line: &str) -> String {
    let mut output = String::new();
    let mut previous_was_space = false;
    let mut in_string = false;
    let mut escaped = false;

    for ch in line.chars() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => {
                output.push(ch);
                in_string = true;
                previous_was_space = false;
            }
            '{' => {
                ensure_single_space_before(&mut output);
                output.push('{');
                previous_was_space = false;
            }
            ':' if output.ends_with(' ') => {
                trim_trailing_space(&mut output);
                output.push(':');
                previous_was_space = false;
            }
            ';' | ',' => {
                trim_trailing_space(&mut output);
                output.push(ch);
                previous_was_space = false;
            }
            ch if ch.is_whitespace() => {
                if !previous_was_space && !output.is_empty() {
                    output.push(' ');
                    previous_was_space = true;
                }
            }
            _ => {
                output.push(ch);
                previous_was_space = false;
            }
        }
    }

    output.trim_end().to_string()
}

fn ensure_single_space_before(output: &mut String) {
    trim_trailing_space(output);
    if !output.is_empty() {
        output.push(' ');
    }
}

fn trim_trailing_space(output: &mut String) {
    while output.ends_with(' ') {
        output.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_nested_sysml_indentation() {
        let formatted =
            format_sysml_text("package   Demo{\npart def Vehicle{\npart engine : Engine ;\n}\n}\n");

        assert_eq!(
            formatted,
            "package Demo {\n  part def Vehicle {\n    part engine: Engine;\n  }\n}\n"
        );
    }

    #[test]
    fn preserves_string_contents() {
        let formatted = format_sysml_text("package Demo { doc about \"A   B { C }\"; }\n");

        assert_eq!(formatted, "package Demo { doc about \"A   B { C }\"; }\n");
    }

    #[test]
    fn rejects_kerml_for_now() {
        assert_eq!(
            format_text("package Demo { }", SourceLanguage::Kerml).unwrap_err(),
            FormatError::UnsupportedLanguage
        );
    }
}
