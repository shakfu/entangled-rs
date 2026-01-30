//! Delimited token extraction.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::text_location::TextLocation;

/// Pattern for matching code fence openings.
static FENCE_OPEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<indent>\s*)(?P<fence>`{3,}|~{3,})(?P<info>.*)$").unwrap());

/// A delimited token extracted from input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelimitedToken {
    /// The info string from the opening delimiter.
    pub info: String,
    /// The content between delimiters.
    pub content: String,
    /// Location of the opening delimiter.
    pub location: TextLocation,
    /// Indentation of the code fence.
    pub indent: String,
}

/// Result of attempting to extract a delimited token.
#[derive(Debug)]
pub enum ExtractResult {
    /// Successfully extracted a token.
    Token(DelimitedToken),
    /// No opening delimiter found, returns the line.
    NotDelimited(String),
    /// Unclosed delimiter (reached end of input).
    Unclosed {
        info: String,
        content: String,
        location: TextLocation,
    },
}

/// Extracts delimited tokens (code blocks) from lines.
pub struct DelimitedTokenGetter {
    /// Current line number (1-indexed).
    line_number: usize,
}

impl DelimitedTokenGetter {
    /// Creates a new getter.
    pub fn new() -> Self {
        Self { line_number: 1 }
    }

    /// Creates a new getter starting at a specific line.
    pub fn at_line(line: usize) -> Self {
        Self { line_number: line }
    }

    /// Extracts the next token from the line iterator.
    pub fn extract<'a, I>(&mut self, lines: &mut I) -> Option<ExtractResult>
    where
        I: Iterator<Item = &'a str>,
    {
        let line = lines.next()?;
        let start_line = self.line_number;
        self.line_number += 1;

        // Check for fence opening
        let Some(caps) = FENCE_OPEN.captures(line) else {
            return Some(ExtractResult::NotDelimited(line.to_string()));
        };

        let indent = caps["indent"].to_string();
        let fence = &caps["fence"];
        let info = caps["info"].trim().to_string();
        let fence_char = fence.chars().next().unwrap();
        let fence_len = fence.len();

        // Build closing pattern: same or more fence chars
        let close_pattern = format!(r"^\s*{}{{{},}}\s*$", fence_char, fence_len);
        let close_regex = Regex::new(&close_pattern).unwrap();

        let mut content_lines = Vec::new();

        // Collect content until closing fence
        loop {
            match lines.next() {
                Some(content_line) => {
                    self.line_number += 1;

                    if close_regex.is_match(content_line) {
                        // Found closing fence
                        let content = content_lines.join("\n");
                        return Some(ExtractResult::Token(DelimitedToken {
                            info,
                            content,
                            location: TextLocation::line_only(start_line),
                            indent,
                        }));
                    }

                    // Strip indent from content if present
                    let stripped = if content_line.starts_with(&indent) {
                        &content_line[indent.len()..]
                    } else {
                        content_line
                    };
                    content_lines.push(stripped.to_string());
                }
                None => {
                    // Reached end without closing fence
                    let content = content_lines.join("\n");
                    return Some(ExtractResult::Unclosed {
                        info,
                        content,
                        location: TextLocation::line_only(start_line),
                    });
                }
            }
        }
    }
}

impl Default for DelimitedTokenGetter {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to extract all tokens from a string.
pub fn extract_all_tokens(input: &str) -> Vec<ExtractResult> {
    let mut getter = DelimitedTokenGetter::new();
    let mut lines = input.lines().peekable();
    let mut results = Vec::new();

    while lines.peek().is_some() {
        if let Some(result) = getter.extract(&mut lines) {
            results.push(result);
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_code_block() {
        let input = "```python\nprint('hello')\n```";
        let results = extract_all_tokens(input);

        assert_eq!(results.len(), 1);
        if let ExtractResult::Token(token) = &results[0] {
            assert_eq!(token.info, "python");
            assert_eq!(token.content, "print('hello')");
            assert_eq!(token.location.line, 1);
        } else {
            panic!("Expected Token");
        }
    }

    #[test]
    fn test_code_block_with_attributes() {
        let input = "```python #main file=out.py\ncode\n```";
        let results = extract_all_tokens(input);

        if let ExtractResult::Token(token) = &results[0] {
            assert_eq!(token.info, "python #main file=out.py");
        } else {
            panic!("Expected Token");
        }
    }

    #[test]
    fn test_tilde_fence() {
        let input = "~~~rust\nfn main() {}\n~~~";
        let results = extract_all_tokens(input);

        if let ExtractResult::Token(token) = &results[0] {
            assert_eq!(token.info, "rust");
            assert_eq!(token.content, "fn main() {}");
        } else {
            panic!("Expected Token");
        }
    }

    #[test]
    fn test_longer_fence() {
        let input = "````python\n```not a fence```\n````";
        let results = extract_all_tokens(input);

        if let ExtractResult::Token(token) = &results[0] {
            assert_eq!(token.content, "```not a fence```");
        } else {
            panic!("Expected Token");
        }
    }

    #[test]
    fn test_not_delimited() {
        let input = "Just some text";
        let results = extract_all_tokens(input);

        assert_eq!(results.len(), 1);
        if let ExtractResult::NotDelimited(text) = &results[0] {
            assert_eq!(text, "Just some text");
        } else {
            panic!("Expected NotDelimited");
        }
    }

    #[test]
    fn test_unclosed_fence() {
        let input = "```python\ncode\nmore code";
        let results = extract_all_tokens(input);

        if let ExtractResult::Unclosed { info, content, .. } = &results[0] {
            assert_eq!(info, "python");
            assert_eq!(content, "code\nmore code");
        } else {
            panic!("Expected Unclosed");
        }
    }

    #[test]
    fn test_indented_fence() {
        let input = "    ```python\n    code\n    ```";
        let results = extract_all_tokens(input);

        if let ExtractResult::Token(token) = &results[0] {
            assert_eq!(token.indent, "    ");
            assert_eq!(token.content, "code");
        } else {
            panic!("Expected Token");
        }
    }

    #[test]
    fn test_multiple_blocks() {
        let input = "text\n```python\ncode1\n```\nmore text\n```rust\ncode2\n```";
        let results = extract_all_tokens(input);

        assert_eq!(results.len(), 4);
        assert!(matches!(&results[0], ExtractResult::NotDelimited(_)));
        assert!(matches!(&results[1], ExtractResult::Token(_)));
        assert!(matches!(&results[2], ExtractResult::NotDelimited(_)));
        assert!(matches!(&results[3], ExtractResult::Token(_)));
    }

    #[test]
    fn test_empty_code_block() {
        let input = "```python\n```";
        let results = extract_all_tokens(input);

        if let ExtractResult::Token(token) = &results[0] {
            assert_eq!(token.content, "");
        } else {
            panic!("Expected Token");
        }
    }

    #[test]
    fn test_multiline_content() {
        let input = "```python\nline1\nline2\nline3\n```";
        let results = extract_all_tokens(input);

        if let ExtractResult::Token(token) = &results[0] {
            assert_eq!(token.content, "line1\nline2\nline3");
        } else {
            panic!("Expected Token");
        }
    }
}
