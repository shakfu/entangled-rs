//! Annotated code reader for stitching.
//!
//! Reads tangled source files with annotation comments and extracts
//! code blocks for updating the original markdown.

use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

use crate::errors::{EntangledError, Result};
use crate::model::{CodeBlock, ReferenceId, ReferenceMap};
use crate::text_location::TextLocation;

/// Pattern for matching annotation begin markers.
static BEGIN_PATTERN: Lazy<Regex> = Lazy::new(|| {
    // Matches: # ~/~ begin <<refid>>
    Regex::new(r"^\s*(?P<prefix>\S+)\s+~/~\s+begin\s+<<(?P<ref>[^>]+)>>").unwrap()
});

/// Pattern for matching annotation end markers.
static END_PATTERN: Lazy<Regex> = Lazy::new(|| {
    // Matches: # ~/~ end
    Regex::new(r"^\s*\S+\s+~/~\s+end\s*$").unwrap()
});

/// A code block extracted from annotated source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotatedBlock {
    /// The reference ID.
    pub id: ReferenceId,
    /// The source code content.
    pub source: String,
    /// Indentation of the block.
    pub indent: String,
    /// Starting line number.
    pub start_line: usize,
    /// Ending line number.
    pub end_line: usize,
}

/// Reads annotated code and extracts blocks.
pub fn read_annotated_code(
    input: &str,
    _source_path: Option<&Path>,
) -> Result<Vec<AnnotatedBlock>> {
    let mut blocks = Vec::new();
    let mut stack: Vec<(ReferenceId, String, usize, Vec<String>)> = Vec::new();

    for (line_num, line) in input.lines().enumerate() {
        let line_number = line_num + 1;

        if let Some(caps) = BEGIN_PATTERN.captures(line) {
            let ref_str = &caps["ref"];
            let id = ReferenceId::parse(ref_str).ok_or_else(|| EntangledError::Parse {
                location: TextLocation::line_only(line_number),
                message: format!("Invalid reference ID: {}", ref_str),
            })?;

            // Calculate indent from the line
            let indent = line
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect::<String>();

            stack.push((id, indent, line_number, Vec::new()));
        } else if END_PATTERN.is_match(line) {
            if let Some((id, indent, start_line, content_lines)) = stack.pop() {
                blocks.push(AnnotatedBlock {
                    id,
                    source: content_lines.join("\n"),
                    indent,
                    start_line,
                    end_line: line_number,
                });
            } else {
                tracing::warn!("Unmatched end marker at line {}", line_number);
            }
        } else if let Some((_, ref indent, _, ref mut content)) = stack.last_mut() {
            // Strip the block's indent from content lines
            let stripped = if line.starts_with(indent.as_str()) {
                &line[indent.len()..]
            } else {
                line
            };
            content.push(stripped.to_string());
        }
    }

    // Check for unclosed blocks
    if !stack.is_empty() {
        let (id, _, start_line, _) = stack.pop().unwrap();
        return Err(EntangledError::Parse {
            location: TextLocation::line_only(start_line),
            message: format!("Unclosed block: {}", id),
        });
    }

    Ok(blocks)
}

/// Reads an annotated code file and returns a reference map.
pub fn read_annotated_file(path: &Path) -> Result<ReferenceMap> {
    let content = std::fs::read_to_string(path)?;
    let blocks = read_annotated_code(&content, Some(path))?;

    let mut refs = ReferenceMap::new();
    for block in blocks {
        let code_block = CodeBlock::new(
            block.id.clone(),
            None, // Language not available from annotations
            block.source,
            TextLocation::file_line(path.to_path_buf(), block.start_line),
        );
        refs.insert_with_id(block.id, code_block);
    }

    Ok(refs)
}

/// Extracts top-level blocks (not nested).
/// For top-level blocks, the content includes any nested annotations.
pub fn read_top_level_blocks(input: &str) -> Result<Vec<AnnotatedBlock>> {
    let mut depth: i32 = 0;
    let mut current_block: Option<(ReferenceId, String, usize, Vec<String>)> = None;
    let mut top_level = Vec::new();

    for (line_num, line) in input.lines().enumerate() {
        let line_number = line_num + 1;

        if let Some(caps) = BEGIN_PATTERN.captures(line) {
            if depth == 0 {
                let ref_str = &caps["ref"];
                if let Some(id) = ReferenceId::parse(ref_str) {
                    let indent = line
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .collect::<String>();
                    current_block = Some((id, indent, line_number, Vec::new()));
                }
            } else if depth >= 1 {
                // Nested begin marker - include it in content
                if let Some((_, ref indent, _, ref mut content)) = current_block.as_mut() {
                    let stripped = if line.starts_with(indent.as_str()) {
                        &line[indent.len()..]
                    } else {
                        line
                    };
                    content.push(stripped.to_string());
                }
            }
            depth += 1;
        } else if END_PATTERN.is_match(line) {
            depth -= 1;
            if depth == 0 {
                if let Some((id, indent, start_line, content_lines)) = current_block.take() {
                    top_level.push(AnnotatedBlock {
                        id,
                        source: content_lines.join("\n"),
                        indent,
                        start_line,
                        end_line: line_number,
                    });
                }
            } else if depth >= 1 {
                // Nested end marker - include it in content
                if let Some((_, ref indent, _, ref mut content)) = current_block.as_mut() {
                    let stripped = if line.starts_with(indent.as_str()) {
                        &line[indent.len()..]
                    } else {
                        line
                    };
                    content.push(stripped.to_string());
                }
            }
        } else if depth >= 1 {
            // Regular content inside a top-level block (at any nesting depth)
            if let Some((_, ref indent, _, ref mut content)) = current_block.as_mut() {
                let stripped = if line.starts_with(indent.as_str()) {
                    &line[indent.len()..]
                } else {
                    line
                };
                content.push(stripped.to_string());
            }
        }
    }

    Ok(top_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_simple_block() {
        let input = r#"# ~/~ begin <<main[0]>>
print('hello')
# ~/~ end
"#;
        let blocks = read_annotated_code(input, None).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id.name.as_str(), "main");
        assert_eq!(blocks[0].id.count, 0);
        assert_eq!(blocks[0].source, "print('hello')");
    }

    #[test]
    fn test_read_indented_block() {
        let input = r#"    # ~/~ begin <<inner[0]>>
    code
    more code
    # ~/~ end
"#;
        let blocks = read_annotated_code(input, None).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].indent, "    ");
        assert_eq!(blocks[0].source, "code\nmore code");
    }

    #[test]
    fn test_read_nested_blocks() {
        let input = r#"# ~/~ begin <<outer[0]>>
def main():
    # ~/~ begin <<inner[0]>>
    pass
    # ~/~ end
# ~/~ end
"#;
        let blocks = read_annotated_code(input, None).unwrap();

        assert_eq!(blocks.len(), 2);
        // Inner block first (closed first)
        assert_eq!(blocks[0].id.name.as_str(), "inner");
        assert_eq!(blocks[1].id.name.as_str(), "outer");
    }

    #[test]
    fn test_read_multiple_blocks() {
        let input = r#"# ~/~ begin <<a[0]>>
code a
# ~/~ end
# ~/~ begin <<b[0]>>
code b
# ~/~ end
"#;
        let blocks = read_annotated_code(input, None).unwrap();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id.name.as_str(), "a");
        assert_eq!(blocks[1].id.name.as_str(), "b");
    }

    #[test]
    fn test_unclosed_block() {
        let input = r#"# ~/~ begin <<main[0]>>
code
"#;
        let result = read_annotated_code(input, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_top_level() {
        let input = r#"# ~/~ begin <<outer[0]>>
def main():
    # ~/~ begin <<inner[0]>>
    pass
    # ~/~ end
# ~/~ end
"#;
        let blocks = read_top_level_blocks(input).unwrap();

        // Should only return the outer block
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id.name.as_str(), "outer");
        // Content should include the nested annotations
        assert!(blocks[0].source.contains("# ~/~ begin <<inner[0]>>"));
    }

    #[test]
    fn test_different_comment_styles() {
        let input = r#"// ~/~ begin <<rust_block[0]>>
fn main() {}
// ~/~ end
"#;
        let blocks = read_annotated_code(input, None).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id.name.as_str(), "rust_block");
    }

    #[test]
    fn test_namespaced_reference() {
        let input = r#"# ~/~ begin <<file.md#main[0]>>
code
# ~/~ end
"#;
        let blocks = read_annotated_code(input, None).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id.name.as_str(), "file.md#main");
    }
}
