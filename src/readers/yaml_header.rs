//! YAML frontmatter extraction.

use crate::text_location::TextLocation;

/// Result of YAML header extraction.
#[derive(Debug, Clone)]
pub struct YamlHeader {
    /// The YAML content (without delimiters).
    pub content: String,
    /// Location in source.
    pub location: TextLocation,
    /// Number of lines consumed (including delimiters).
    pub lines_consumed: usize,
}

/// Extracts YAML frontmatter from the beginning of a document.
///
/// YAML frontmatter is delimited by `---` at the start and end.
/// Returns None if no valid frontmatter is found.
pub fn extract_yaml_header(input: &str) -> Option<YamlHeader> {
    let mut lines = input.lines().peekable();

    // First line must be ---
    let first = lines.next()?;
    if first.trim() != "---" {
        return None;
    }

    let mut content_lines = Vec::new();
    let mut line_count = 1;

    // Collect until closing ---
    for line in lines {
        line_count += 1;
        if line.trim() == "---" {
            return Some(YamlHeader {
                content: content_lines.join("\n"),
                location: TextLocation::line_only(1),
                lines_consumed: line_count,
            });
        }
        content_lines.push(line);
    }

    // No closing delimiter found
    None
}

/// Splits a document into YAML header and remaining content.
pub fn split_yaml_header(input: &str) -> (Option<YamlHeader>, &str) {
    match extract_yaml_header(input) {
        Some(header) => {
            // Find where the remaining content starts
            let mut pos = 0;
            let mut line_count = 0;
            for line in input.lines() {
                line_count += 1;
                pos += line.len() + 1; // +1 for newline
                if line_count >= header.lines_consumed {
                    break;
                }
            }
            // Handle case where input might not have trailing newline
            let remaining = if pos <= input.len() {
                &input[pos.min(input.len())..]
            } else {
                ""
            };
            (Some(header), remaining)
        }
        None => (None, input),
    }
}

/// Parses YAML header content into key-value pairs.
///
/// This is a simple parser for basic YAML (key: value pairs).
pub fn parse_simple_yaml(content: &str) -> std::collections::HashMap<String, String> {
    let mut result = std::collections::HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim();

            // Remove quotes if present
            let value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value[1..value.len() - 1].to_string()
            } else {
                value.to_string()
            };

            result.insert(key, value);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_yaml_header() {
        let input = "---\ntitle: Test\nauthor: Me\n---\n# Content";
        let header = extract_yaml_header(input).unwrap();

        assert_eq!(header.content, "title: Test\nauthor: Me");
        assert_eq!(header.lines_consumed, 4);
    }

    #[test]
    fn test_no_yaml_header() {
        let input = "# Just markdown\nNo frontmatter";
        assert!(extract_yaml_header(input).is_none());
    }

    #[test]
    fn test_unclosed_yaml() {
        let input = "---\ntitle: Test\nauthor: Me";
        assert!(extract_yaml_header(input).is_none());
    }

    #[test]
    fn test_split_yaml_header() {
        let input = "---\ntitle: Test\n---\n# Content\nMore";
        let (header, remaining) = split_yaml_header(input);

        assert!(header.is_some());
        assert_eq!(remaining.trim(), "# Content\nMore");
    }

    #[test]
    fn test_split_no_header() {
        let input = "# Content\nMore";
        let (header, remaining) = split_yaml_header(input);

        assert!(header.is_none());
        assert_eq!(remaining, input);
    }

    #[test]
    fn test_parse_simple_yaml() {
        let yaml = "title: My Document\nauthor: John Doe\nversion: 1.0";
        let parsed = parse_simple_yaml(yaml);

        assert_eq!(parsed.get("title"), Some(&"My Document".to_string()));
        assert_eq!(parsed.get("author"), Some(&"John Doe".to_string()));
        assert_eq!(parsed.get("version"), Some(&"1.0".to_string()));
    }

    #[test]
    fn test_parse_yaml_with_quotes() {
        let yaml = "title: \"Quoted Title\"\nother: 'Single quoted'";
        let parsed = parse_simple_yaml(yaml);

        assert_eq!(parsed.get("title"), Some(&"Quoted Title".to_string()));
        assert_eq!(parsed.get("other"), Some(&"Single quoted".to_string()));
    }

    #[test]
    fn test_parse_yaml_with_comments() {
        let yaml = "# Comment\ntitle: Test\n# Another comment\nauthor: Me";
        let parsed = parse_simple_yaml(yaml);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.get("title"), Some(&"Test".to_string()));
    }

    #[test]
    fn test_empty_yaml() {
        let yaml = "";
        let parsed = parse_simple_yaml(yaml);
        assert!(parsed.is_empty());
    }
}
