//! Quarto property parsing.

use super::{Properties, Property, strip_quotes};

/// Quarto options extracted from `#|` comment lines.
#[derive(Debug, Clone, Default)]
pub struct QuartoOptions {
    /// The label/ID for the code block.
    pub label: Option<String>,
    /// The output file target.
    pub file: Option<String>,
    /// Other options as key-value pairs.
    pub other: Vec<(String, String)>,
}

impl QuartoOptions {
    /// Creates a new empty QuartoOptions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if any options are set.
    pub fn is_empty(&self) -> bool {
        self.label.is_none() && self.file.is_none() && self.other.is_empty()
    }

    /// Sets an option by key.
    pub fn set(&mut self, key: &str, value: String) {
        match key {
            "label" => self.label = Some(value),
            "file" => self.file = Some(value),
            _ => self.other.push((key.to_string(), value)),
        }
    }

    /// Converts to Properties, optionally with a language.
    pub fn to_properties(&self, language: Option<&str>) -> Properties {
        let mut items = Vec::new();

        if let Some(lang) = language {
            items.push(Property::Class(lang.to_string()));
        }

        if let Some(ref label) = self.label {
            items.push(Property::Id(label.clone()));
        }

        if let Some(ref file) = self.file {
            items.push(Property::Attribute("file".to_string(), file.clone()));
        }

        for (key, value) in &self.other {
            items.push(Property::Attribute(key.clone(), value.clone()));
        }

        Properties::new(items)
    }
}

/// Extract `#|` options from Quarto-style code block content.
///
/// Returns the extracted options and the remaining content (with #| lines removed).
pub fn extract_quarto_options(content: &str) -> (QuartoOptions, String) {
    let mut options = QuartoOptions::new();
    let mut remaining_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("#|") {
            // Parse the option: "key: value" or "key=value"
            let rest = rest.trim();
            if let Some((key, value)) = parse_quarto_option_line(rest) {
                options.set(&key, value);
            }
        } else {
            remaining_lines.push(line);
        }
    }

    (options, remaining_lines.join("\n"))
}

/// Parse a single Quarto option line (after the `#|` prefix).
/// Supports both `key: value` (YAML) and `key=value` formats.
fn parse_quarto_option_line(line: &str) -> Option<(String, String)> {
    // Try YAML-style "key: value" first
    if let Some((key, value)) = line.split_once(':') {
        let key = key.trim();
        let value = value.trim();
        if !key.is_empty() {
            return Some((key.to_string(), strip_quotes(value).to_string()));
        }
    }
    // Try "key=value" format
    if let Some((key, value)) = line.split_once('=') {
        let key = key.trim();
        let value = value.trim();
        if !key.is_empty() {
            return Some((key.to_string(), strip_quotes(value).to_string()));
        }
    }
    None
}
