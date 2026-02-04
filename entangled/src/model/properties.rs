//! Property parsing for code block attributes.
//!
//! Parses property strings like `.python #main file=output.py mode=0755`
//! into structured Property values.

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{escaped_transform, tag, take_while1},
    character::complete::{char, multispace0, multispace1, none_of},
    combinator::{map, opt, value},
    multi::many0,
    sequence::{delimited, preceded},
};

/// A single property from a code block header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Property {
    /// A class property, e.g., `.python`
    Class(String),
    /// An ID property, e.g., `#main`
    Id(String),
    /// A key-value attribute, e.g., `file="output.py"`
    Attribute(String, String),
}

impl Property {
    /// Returns true if this is a class property.
    pub fn is_class(&self) -> bool {
        matches!(self, Property::Class(_))
    }

    /// Returns true if this is an ID property.
    pub fn is_id(&self) -> bool {
        matches!(self, Property::Id(_))
    }

    /// Returns true if this is an attribute property.
    pub fn is_attribute(&self) -> bool {
        matches!(self, Property::Attribute(_, _))
    }

    /// Returns the class name if this is a class property.
    pub fn as_class(&self) -> Option<&str> {
        match self {
            Property::Class(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the ID if this is an ID property.
    pub fn as_id(&self) -> Option<&str> {
        match self {
            Property::Id(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the key-value pair if this is an attribute.
    pub fn as_attribute(&self) -> Option<(&str, &str)> {
        match self {
            Property::Attribute(k, v) => Some((k, v)),
            _ => None,
        }
    }
}

/// Check if a character is valid in an identifier.
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == ':' || c == '/' || c == '.'
}

/// Parse an identifier (alphanumeric, underscore, dash, colon, slash, dot).
fn parse_ident(input: &str) -> IResult<&str, &str> {
    take_while1(is_ident_char).parse(input)
}

/// Parse a class property: `.classname`
fn parse_class(input: &str) -> IResult<&str, Property> {
    map(preceded(char('.'), parse_ident), |s: &str| {
        Property::Class(s.to_string())
    }).parse(input)
}

/// Parse an ID property: `#idname`
fn parse_id(input: &str) -> IResult<&str, Property> {
    map(preceded(char('#'), parse_ident), |s: &str| {
        Property::Id(s.to_string())
    }).parse(input)
}

/// Parse a quoted string value with escape handling.
fn parse_quoted_string(input: &str) -> IResult<&str, String> {
    delimited(
        char('"'),
        escaped_transform(
            none_of("\\\""),
            '\\',
            alt((
                value("\\", tag("\\")),
                value("\"", tag("\"")),
                value("\n", tag("n")),
                value("\t", tag("t")),
                value("\r", tag("r")),
            )),
        ),
        char('"'),
    ).parse(input)
}

/// Parse an unquoted value (no spaces or special chars).
fn parse_unquoted_value(input: &str) -> IResult<&str, String> {
    map(take_while1(is_ident_char), |s: &str| {
        s.to_string()
    }).parse(input)
}

/// Parse an attribute value (quoted or unquoted).
fn parse_value(input: &str) -> IResult<&str, String> {
    alt((parse_quoted_string, parse_unquoted_value)).parse(input)
}

/// Parse an attribute: `key=value` or `key="value"`
fn parse_attribute(input: &str) -> IResult<&str, Property> {
    map(
        (parse_ident, char('='), parse_value),
        |(key, _, val)| Property::Attribute(key.to_string(), val),
    ).parse(input)
}

/// Parse a plain language identifier (first word without prefix).
fn parse_plain_class(input: &str) -> IResult<&str, Property> {
    map(parse_ident, |s: &str| {
        Property::Class(s.to_string())
    }).parse(input)
}

/// Parse a single property (class, id, or attribute).
fn parse_property(input: &str) -> IResult<&str, Property> {
    alt((parse_class, parse_id, parse_attribute)).parse(input)
}

/// Parse a property that could be a plain word (language) or prefixed property.
fn parse_any_property(input: &str) -> IResult<&str, Property> {
    alt((parse_class, parse_id, parse_attribute, parse_plain_class)).parse(input)
}

/// Parse multiple properties separated by whitespace.
/// The first property can be a plain identifier (language), rest must be prefixed.
fn parse_properties_inner(input: &str) -> IResult<&str, Vec<Property>> {
    let (input, _) = multispace0.parse(input)?;
    // First property can be a plain word (language identifier)
    let (input, first) = opt(parse_any_property).parse(input)?;

    match first {
        None => Ok((input, vec![])),
        Some(prop) => {
            // Subsequent properties must be prefixed (., #, or key=)
            let (input, rest) = many0(preceded(multispace1, parse_property)).parse(input)?;
            let mut props = vec![prop];
            props.extend(rest);
            let (input, _) = multispace0.parse(input)?;
            Ok((input, props))
        }
    }
}

/// Parse a property string into a list of properties.
pub fn parse_properties(input: &str) -> Result<Vec<Property>, String> {
    match parse_properties_inner(input) {
        Ok(("", props)) => Ok(props),
        Ok((remaining, _)) => Err(format!("Unexpected input: '{}'", remaining)),
        Err(e) => Err(format!("Parse error: {}", e)),
    }
}

/// Parsed properties with convenient accessors.
#[derive(Debug, Clone, Default)]
pub struct Properties {
    /// All properties in order.
    pub items: Vec<Property>,
}

impl Properties {
    /// Creates a new Properties from a list of Property items.
    pub fn new(items: Vec<Property>) -> Self {
        Self { items }
    }

    /// Parses a property string.
    pub fn parse(input: &str) -> Result<Self, String> {
        Ok(Self::new(parse_properties(input)?))
    }

    /// Returns all class names.
    pub fn classes(&self) -> Vec<&str> {
        self.items.iter().filter_map(|p| p.as_class()).collect()
    }

    /// Returns the first class (typically the language).
    pub fn first_class(&self) -> Option<&str> {
        self.items.iter().find_map(|p| p.as_class())
    }

    /// Returns all IDs.
    pub fn ids(&self) -> Vec<&str> {
        self.items.iter().filter_map(|p| p.as_id()).collect()
    }

    /// Returns the first ID.
    pub fn first_id(&self) -> Option<&str> {
        self.items.iter().find_map(|p| p.as_id())
    }

    /// Returns all attributes as key-value pairs.
    pub fn attributes(&self) -> Vec<(&str, &str)> {
        self.items.iter().filter_map(|p| p.as_attribute()).collect()
    }

    /// Gets an attribute value by key.
    pub fn get_attribute(&self, key: &str) -> Option<&str> {
        self.items.iter().find_map(|p| match p {
            Property::Attribute(k, v) if k == key => Some(v.as_str()),
            _ => None,
        })
    }

    /// Returns the file attribute if present.
    pub fn file(&self) -> Option<&str> {
        self.get_attribute("file")
    }

    /// Parses a Pandoc-style info string: `{.python #main file=out.py}`.
    /// Strips outer braces and uses the standard parser.
    pub fn parse_pandoc(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        let inner = strip_braces(trimmed);
        Self::parse(inner)
    }

    /// Parses a knitr-style info string: `{python, label=main, file=out.py}`.
    /// Handles comma-separated options and converts `label=x` to an ID.
    pub fn parse_knitr(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        let inner = strip_braces(trimmed);
        parse_knitr_properties(inner)
    }

    /// Parses a Quarto-style info string: `{python}`.
    /// Only extracts the language; options come from content.
    pub fn parse_quarto_info(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        let inner = strip_braces(trimmed);
        let lang = inner.trim();
        if lang.is_empty() {
            Ok(Self::new(vec![]))
        } else {
            Ok(Self::new(vec![Property::Class(lang.to_string())]))
        }
    }
}

/// Strip outer braces from a string like `{content}`.
fn strip_braces(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('{') && s.ends_with('}') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Parse knitr-style comma-separated properties.
/// Format: `python, label=main, file=out.py, echo=FALSE`
fn parse_knitr_properties(input: &str) -> Result<Properties, String> {
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

/// Split knitr options by comma, respecting quoted values.
fn split_knitr_options(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
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

/// Strip surrounding quotes from a value.
fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_class() {
        let props = parse_properties(".python").unwrap();
        assert_eq!(props, vec![Property::Class("python".to_string())]);
    }

    #[test]
    fn test_parse_id() {
        let props = parse_properties("#main").unwrap();
        assert_eq!(props, vec![Property::Id("main".to_string())]);
    }

    #[test]
    fn test_parse_attribute_unquoted() {
        let props = parse_properties("file=output.py").unwrap();
        assert_eq!(
            props,
            vec![Property::Attribute("file".to_string(), "output.py".to_string())]
        );
    }

    #[test]
    fn test_parse_attribute_quoted() {
        let props = parse_properties("file=\"output file.py\"").unwrap();
        assert_eq!(
            props,
            vec![Property::Attribute("file".to_string(), "output file.py".to_string())]
        );
    }

    #[test]
    fn test_parse_multiple() {
        let props = parse_properties(".python #main file=output.py").unwrap();
        assert_eq!(
            props,
            vec![
                Property::Class("python".to_string()),
                Property::Id("main".to_string()),
                Property::Attribute("file".to_string(), "output.py".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_with_extra_whitespace() {
        let props = parse_properties("  .python   #main  ").unwrap();
        assert_eq!(
            props,
            vec![
                Property::Class("python".to_string()),
                Property::Id("main".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_escaped_quotes() {
        let props = parse_properties("desc=\"hello \\\"world\\\"\"").unwrap();
        assert_eq!(
            props,
            vec![Property::Attribute("desc".to_string(), "hello \"world\"".to_string())]
        );
    }

    #[test]
    fn test_properties_accessors() {
        let props = Properties::parse(".python .module #main #alt file=out.py mode=0755").unwrap();

        assert_eq!(props.classes(), vec!["python", "module"]);
        assert_eq!(props.first_class(), Some("python"));
        assert_eq!(props.ids(), vec!["main", "alt"]);
        assert_eq!(props.first_id(), Some("main"));
        assert_eq!(props.file(), Some("out.py"));
        assert_eq!(props.get_attribute("mode"), Some("0755"));
        assert_eq!(props.get_attribute("nonexistent"), None);
    }

    #[test]
    fn test_empty_input() {
        let props = parse_properties("").unwrap();
        assert!(props.is_empty());
    }

    #[test]
    fn test_namespaced_id() {
        let props = parse_properties("#module::function").unwrap();
        assert_eq!(props, vec![Property::Id("module::function".to_string())]);
    }

    #[test]
    fn test_file_path_with_slashes() {
        let props = parse_properties("file=src/lib/output.rs").unwrap();
        assert_eq!(
            props,
            vec![Property::Attribute("file".to_string(), "src/lib/output.rs".to_string())]
        );
    }

    // Pandoc style tests
    #[test]
    fn test_pandoc_simple() {
        let props = Properties::parse_pandoc("{.python}").unwrap();
        assert_eq!(props.first_class(), Some("python"));
    }

    #[test]
    fn test_pandoc_with_id() {
        let props = Properties::parse_pandoc("{.python #main}").unwrap();
        assert_eq!(props.first_class(), Some("python"));
        assert_eq!(props.first_id(), Some("main"));
    }

    #[test]
    fn test_pandoc_full() {
        let props = Properties::parse_pandoc("{.python #main file=out.py}").unwrap();
        assert_eq!(props.first_class(), Some("python"));
        assert_eq!(props.first_id(), Some("main"));
        assert_eq!(props.file(), Some("out.py"));
    }

    #[test]
    fn test_pandoc_with_whitespace() {
        let props = Properties::parse_pandoc("  {.python #main}  ").unwrap();
        assert_eq!(props.first_class(), Some("python"));
        assert_eq!(props.first_id(), Some("main"));
    }

    // Knitr style tests
    #[test]
    fn test_knitr_simple() {
        let props = Properties::parse_knitr("{python}").unwrap();
        assert_eq!(props.first_class(), Some("python"));
    }

    #[test]
    fn test_knitr_with_label() {
        let props = Properties::parse_knitr("{python, label=main}").unwrap();
        assert_eq!(props.first_class(), Some("python"));
        assert_eq!(props.first_id(), Some("main"));
    }

    #[test]
    fn test_knitr_full() {
        let props = Properties::parse_knitr("{python, label=main, file=out.py}").unwrap();
        assert_eq!(props.first_class(), Some("python"));
        assert_eq!(props.first_id(), Some("main"));
        assert_eq!(props.file(), Some("out.py"));
    }

    #[test]
    fn test_knitr_with_quoted_values() {
        let props = Properties::parse_knitr("{r, label=\"my-chunk\", file=\"path/to/file.R\"}").unwrap();
        assert_eq!(props.first_class(), Some("r"));
        assert_eq!(props.first_id(), Some("my-chunk"));
        assert_eq!(props.file(), Some("path/to/file.R"));
    }

    #[test]
    fn test_knitr_boolean_flags() {
        let props = Properties::parse_knitr("{r, echo=FALSE, eval=TRUE}").unwrap();
        assert_eq!(props.get_attribute("echo"), Some("FALSE"));
        assert_eq!(props.get_attribute("eval"), Some("TRUE"));
    }

    // Quarto style tests
    #[test]
    fn test_quarto_info_simple() {
        let props = Properties::parse_quarto_info("{python}").unwrap();
        assert_eq!(props.first_class(), Some("python"));
        assert_eq!(props.first_id(), None);
    }

    #[test]
    fn test_quarto_options_extraction() {
        let content = "#| label: main\n#| file: out.py\nprint('hello')\nprint('world')";
        let (opts, remaining) = extract_quarto_options(content);

        assert_eq!(opts.label, Some("main".to_string()));
        assert_eq!(opts.file, Some("out.py".to_string()));
        assert_eq!(remaining, "print('hello')\nprint('world')");
    }

    #[test]
    fn test_quarto_options_yaml_style() {
        let content = "#| label: my-block\n#| eval: false\ncode here";
        let (opts, remaining) = extract_quarto_options(content);

        assert_eq!(opts.label, Some("my-block".to_string()));
        assert_eq!(opts.other, vec![("eval".to_string(), "false".to_string())]);
        assert_eq!(remaining, "code here");
    }

    #[test]
    fn test_quarto_options_equals_style() {
        let content = "#| label=main\n#| file=out.py\ncode";
        let (opts, remaining) = extract_quarto_options(content);

        assert_eq!(opts.label, Some("main".to_string()));
        assert_eq!(opts.file, Some("out.py".to_string()));
        assert_eq!(remaining, "code");
    }

    #[test]
    fn test_quarto_options_to_properties() {
        let mut opts = QuartoOptions::new();
        opts.label = Some("main".to_string());
        opts.file = Some("out.py".to_string());
        opts.other.push(("mode".to_string(), "0755".to_string()));

        let props = opts.to_properties(Some("python"));
        assert_eq!(props.first_class(), Some("python"));
        assert_eq!(props.first_id(), Some("main"));
        assert_eq!(props.file(), Some("out.py"));
        assert_eq!(props.get_attribute("mode"), Some("0755"));
    }

    #[test]
    fn test_quarto_options_empty() {
        let content = "print('hello')\nprint('world')";
        let (opts, remaining) = extract_quarto_options(content);

        assert!(opts.is_empty());
        assert_eq!(remaining, content);
    }

    #[test]
    fn test_quarto_options_with_indented_code() {
        let content = "#| label: main\n    indented code\n    more indented";
        let (opts, remaining) = extract_quarto_options(content);

        assert_eq!(opts.label, Some("main".to_string()));
        assert_eq!(remaining, "    indented code\n    more indented");
    }

    #[test]
    fn test_quarto_options_quoted_values() {
        let content = "#| label: \"my label\"\n#| file: 'out.py'\ncode";
        let (opts, _) = extract_quarto_options(content);

        assert_eq!(opts.label, Some("my label".to_string()));
        assert_eq!(opts.file, Some("out.py".to_string()));
    }
}
