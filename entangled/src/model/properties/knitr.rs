//! Knitr/RMarkdown property parsing.

use super::{Properties, Property, strip_braces, strip_quotes};

/// Parse knitr-style comma-separated properties.
/// Format: `python, label=main, file=out.py, echo=FALSE`
pub(crate) fn parse_knitr_properties(input: &str) -> crate::errors::Result<Properties> {
    let mut items = Vec::new();
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Ok(Properties::new(items));
    }

    // Split by comma, but handle quoted values
    let parts = split_knitr_options(trimmed);

    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if i == 0 && !part.contains('=') {
            // First item without `=` is the language
            items.push(Property::Class(part.to_string()));
        } else if let Some((key, value)) = part.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            // Strip quotes if present
            let value = strip_quotes(value);

            // Convert knitr-specific keys
            match key {
                "label" => items.push(Property::Id(value.to_string())),
                _ => items.push(Property::Attribute(key.to_string(), value.to_string())),
            }
        } else {
            // Boolean flag without value (e.g., `eval` is treated as `eval=TRUE`)
            items.push(Property::Attribute(part.to_string(), "true".to_string()));
        }
    }

    Ok(Properties::new(items))
}

/// Parses a knitr-style info string: `{python, label=main, file=out.py}`.
/// Handles comma-separated options and converts `label=x` to an ID.
pub(crate) fn parse_knitr(input: &str) -> crate::errors::Result<Properties> {
    let trimmed = input.trim();
    let inner = strip_braces(trimmed);
    parse_knitr_properties(inner)
}

/// Split knitr options by comma, respecting quoted values.
fn split_knitr_options(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let chars = input.chars();

    for c in chars {
        match c {
            '"' if !in_quotes => {
                in_quotes = true;
                current.push(c);
            }
            '"' if in_quotes => {
                in_quotes = false;
                current.push(c);
            }
            ',' if !in_quotes => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }

    if !current.is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}
