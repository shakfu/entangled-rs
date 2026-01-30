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
}
