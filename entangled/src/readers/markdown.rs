//! Markdown parsing for code block extraction.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::errors::Result;
use crate::model::{CodeBlock, Properties, ReferenceId, ReferenceMap, ReferenceName, extract_quarto_options};
use crate::style::Style;
use crate::text_location::TextLocation;

use super::delimiters::{extract_all_tokens, DelimitedToken, ExtractResult};
use super::yaml_header::split_yaml_header;

/// A parsed markdown document.
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    /// The reference map containing all code blocks.
    pub refs: ReferenceMap,
    /// YAML frontmatter, if present.
    pub frontmatter: Option<String>,
    /// Source file path.
    pub source_path: Option<PathBuf>,
}

impl ParsedDocument {
    /// Creates a new empty parsed document.
    #[must_use]
    pub fn new() -> Self {
        Self {
            refs: ReferenceMap::new(),
            frontmatter: None,
            source_path: None,
        }
    }

    /// Sets the source path.
    pub fn with_source_path(mut self, path: PathBuf) -> Self {
        self.source_path = Some(path);
        self
    }
}

impl Default for ParsedDocument {
    fn default() -> Self {
        Self::new()
    }
}

/// Parses a markdown document and extracts code blocks.
pub fn parse_markdown(input: &str, source_path: Option<&Path>, config: &Config) -> Result<ParsedDocument> {
    let mut doc = ParsedDocument::new();

    if let Some(path) = source_path {
        doc.source_path = Some(path.to_path_buf());
    }

    // Determine document style based on file extension, with config default as fallback
    let doc_style = Style::for_document(source_path, config.style);

    // Extract YAML frontmatter
    let (yaml_header, content) = split_yaml_header(input);
    if let Some(header) = yaml_header {
        doc.frontmatter = Some(header.content);
    }

    // Parse code blocks
    let tokens = extract_all_tokens(content);

    for result in tokens {
        if let ExtractResult::Token(token) = result {
            if let Some(block) = process_code_block(&token, source_path, config, doc_style)? {
                doc.refs.insert(block);
            }
        }
    }

    Ok(doc)
}

/// Processes a delimited token into a CodeBlock.
fn process_code_block(
    token: &DelimitedToken,
    source_path: Option<&Path>,
    config: &Config,
    doc_style: Style,
) -> Result<Option<CodeBlock>> {
    // Parse properties and content based on document style
    let (props, content) = parse_by_style(doc_style, &token.info, &token.content, config)?;

    // Get language from first class
    let language = props.first_class().map(|s| s.to_string());

    // Skip blocks without an ID or file target (anonymous blocks)
    let id_str = props.first_id();
    let file_target = props.file();

    if id_str.is_none() && file_target.is_none() {
        // Anonymous block, skip it
        return Ok(None);
    }

    // Determine the reference name - prioritize explicit ID over file target
    let name = if let Some(id) = id_str {
        // Apply namespace if configured
        let name = if let Some(ns_prefix) = source_path
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .and_then(|n| config.namespace_default.prefix_for(n))
        {
            format!("{}#{}", ns_prefix, id)
        } else {
            id.to_string()
        };
        ReferenceName::new(name)
    } else if let Some(file) = file_target {
        ReferenceName::from_file_path(file)
    } else {
        return Err(crate::errors::EntangledError::Other(
            "Internal error: code block has neither ID nor file target after guard check".to_string(),
        ));
    };

    // Build location
    let location = if let Some(path) = source_path {
        TextLocation::file_line(path.to_path_buf(), token.location.line)
    } else {
        token.location.clone()
    };

    // Create the code block
    let mut block = CodeBlock::new(
        ReferenceId::first(name),
        language,
        content,
        location,
    );

    // Set target if specified
    if let Some(file) = file_target {
        block.target = Some(PathBuf::from(file));
    }

    // Add additional classes
    for class in props.classes().into_iter().skip(1) {
        block = block.with_class(class.to_string());
    }

    // Add attributes
    for (key, value) in props.attributes() {
        if key != "file" {
            block = block.with_attribute(key.to_string(), value.to_string());
        }
    }

    Ok(Some(block))
}

/// Parse properties and content based on the detected style.
fn parse_by_style(
    style: Style,
    info: &str,
    content: &str,
    config: &Config,
) -> Result<(Properties, String)> {
    match style {
        Style::EntangledRs => {
            let props = Properties::parse(info)?;
            Ok((props, content.to_string()))
        }
        Style::Pandoc => {
            let props = Properties::parse_pandoc(info)?;
            Ok((props, content.to_string()))
        }
        Style::Knitr => {
            let props = Properties::parse_knitr(info)?;
            Ok((props, content.to_string()))
        }
        Style::Quarto => {
            // Extract language from info string
            let info_props = Properties::parse_quarto_info(info)?;
            let language = info_props.first_class();

            // Extract #| options from content
            let (quarto_opts, remaining_content) = extract_quarto_options(content);

            // Merge language with quarto options
            let props = quarto_opts.to_properties(language);

            // Determine final content based on strip_quarto_options setting
            let final_content = if config.strip_quarto_options {
                remaining_content
            } else {
                content.to_string()
            };

            Ok((props, final_content))
        }
    }
}

/// Reads a markdown file and parses it.
pub fn read_markdown_file(path: &Path, config: &Config) -> Result<ParsedDocument> {
    let content = std::fs::read_to_string(path)?;
    parse_markdown(&content, Some(path), config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_parse_simple_block() {
        let input = r#"
# Test

```python #main
print('hello')
```
"#;
        let doc = parse_markdown(input, None, &default_config()).unwrap();

        assert_eq!(doc.refs.len(), 1);
        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].source, "print('hello')");
    }

    #[test]
    fn test_parse_with_file_target() {
        let input = r#"
```python file=output.py
print('hello')
```
"#;
        let doc = parse_markdown(input, None, &default_config()).unwrap();

        assert_eq!(doc.refs.len(), 1);
        let blocks = doc.refs.get_by_name(&ReferenceName::from_file_path("output.py"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].target, Some(PathBuf::from("output.py")));
    }

    #[test]
    fn test_skip_anonymous_block() {
        let input = r#"
```python
print('anonymous')
```
"#;
        let doc = parse_markdown(input, None, &default_config()).unwrap();
        assert_eq!(doc.refs.len(), 0);
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let input = r#"
```python #a
block a
```

```python #b
block b
```

```python #a
more a
```
"#;
        let doc = parse_markdown(input, None, &default_config()).unwrap();

        assert_eq!(doc.refs.len(), 3);

        let a_blocks = doc.refs.get_by_name(&ReferenceName::new("a"));
        assert_eq!(a_blocks.len(), 2);
    }

    #[test]
    fn test_parse_with_yaml_frontmatter() {
        let input = r#"---
title: Test Document
---

```python #main
code
```
"#;
        let doc = parse_markdown(input, None, &default_config()).unwrap();

        assert!(doc.frontmatter.is_some());
        assert_eq!(doc.frontmatter.unwrap().trim(), "title: Test Document");
        assert_eq!(doc.refs.len(), 1);
    }

    #[test]
    fn test_parse_with_attributes() {
        let input = r#"
```python #main file=out.py mode=0755
code
```
"#;
        let doc = parse_markdown(input, None, &default_config()).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].target, Some(PathBuf::from("out.py")));
        assert_eq!(blocks[0].get_attribute("mode"), Some("0755"));
    }

    #[test]
    fn test_namespace_default() {
        let input = r#"
```python #main
code
```
"#;
        let path = Path::new("test.md");
        let config = Config::default();

        let doc = parse_markdown(input, Some(path), &config).unwrap();

        // With file namespace default, ID should be prefixed
        let blocks = doc.refs.get_by_name(&ReferenceName::new("test.md#main"));
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn test_location_tracking() {
        let input = r#"# Header

Some text

```python #main
code
```
"#;
        let path = Path::new("test.md");
        let doc = parse_markdown(input, Some(path), &default_config()).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("test.md#main"));
        assert_eq!(blocks[0].location.line, 5);
        assert_eq!(
            blocks[0].location.filename,
            Some(PathBuf::from("test.md"))
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::config::NamespaceDefault;

    #[test]
    fn test_parse_with_references() {
        let input = r#"
```python #main file=program.py
<<imports>>
<<functions>>
```

```python #imports
import sys
```

```python #functions
def main():
    pass
```
"#;
        let mut config = Config::default();
        config.namespace_default = NamespaceDefault::None;

        let doc = parse_markdown(input, None, &config).unwrap();

        // Should have 3 blocks
        assert_eq!(doc.refs.len(), 3, "Expected 3 blocks, got {}", doc.refs.len());

        // Check each block exists
        assert!(doc.refs.contains_name(&ReferenceName::new("main")), "main not found");
        assert!(doc.refs.contains_name(&ReferenceName::new("imports")), "imports not found");
        assert!(doc.refs.contains_name(&ReferenceName::new("functions")), "functions not found");
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;
    use crate::config::NamespaceDefault;
    use crate::style::Style;

    fn config_with_style(style: Style) -> Config {
        let mut config = Config::default();
        config.namespace_default = NamespaceDefault::None;
        config.style = style;
        config
    }

    // EntangledRs style tests
    #[test]
    fn test_entangled_rs_style() {
        let input = r#"
```python #main file=out.py
print('hello')
```
"#;
        let config = config_with_style(Style::EntangledRs);
        let doc = parse_markdown(input, None, &config).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("python".to_string()));
        assert_eq!(blocks[0].target, Some(PathBuf::from("out.py")));
        assert_eq!(blocks[0].source, "print('hello')");
    }

    // Pandoc style tests
    #[test]
    fn test_pandoc_style_simple() {
        let input = r#"
``` {.python #main}
print('hello')
```
"#;
        let config = config_with_style(Style::Pandoc);
        let doc = parse_markdown(input, None, &config).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("python".to_string()));
    }

    #[test]
    fn test_pandoc_style_with_file() {
        let input = r#"
``` {.python #main file=out.py}
print('hello')
```
"#;
        let config = config_with_style(Style::Pandoc);
        let doc = parse_markdown(input, None, &config).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].target, Some(PathBuf::from("out.py")));
    }

    #[test]
    fn test_pandoc_via_config() {
        // With Pandoc configured as default, pandoc syntax works
        let input = r#"
``` {.python #main}
print('hello')
```
"#;
        let config = config_with_style(Style::Pandoc);
        let doc = parse_markdown(input, None, &config).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("python".to_string()));
    }

    // Knitr style tests
    #[test]
    fn test_knitr_style_simple() {
        let input = r#"
```{python, label=main}
print('hello')
```
"#;
        let config = config_with_style(Style::Knitr);
        let doc = parse_markdown(input, None, &config).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("python".to_string()));
    }

    #[test]
    fn test_knitr_style_with_file() {
        let input = r#"
```{r, label=main, file=out.R}
print("hello")
```
"#;
        let config = config_with_style(Style::Knitr);
        let doc = parse_markdown(input, None, &config).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("r".to_string()));
        assert_eq!(blocks[0].target, Some(PathBuf::from("out.R")));
    }

    #[test]
    fn test_knitr_via_extension() {
        // .Rmd files automatically use Knitr style
        let input = r#"
```{python, label=main, file=out.py}
print('hello')
```
"#;
        let config = config_with_style(Style::EntangledRs);
        let path = Path::new("doc.Rmd");
        let doc = parse_markdown(input, Some(path), &config).unwrap();

        // namespace_default is None, so no prefix
        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
    }

    // Quarto style tests
    #[test]
    fn test_quarto_style_simple() {
        let input = r#"
```{python}
#| label: main
print('hello')
```
"#;
        let config = config_with_style(Style::Quarto);
        let doc = parse_markdown(input, None, &config).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, Some("python".to_string()));
        // Content should have #| lines stripped by default
        assert_eq!(blocks[0].source, "print('hello')");
    }

    #[test]
    fn test_quarto_style_with_file() {
        let input = r#"
```{python}
#| label: main
#| file: out.py
print('hello')
```
"#;
        let config = config_with_style(Style::Quarto);
        let doc = parse_markdown(input, None, &config).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].target, Some(PathBuf::from("out.py")));
        assert_eq!(blocks[0].source, "print('hello')");
    }

    #[test]
    fn test_quarto_preserve_options() {
        let input = r#"
```{python}
#| label: main
print('hello')
```
"#;
        let mut config = config_with_style(Style::Quarto);
        config.strip_quarto_options = false;
        let doc = parse_markdown(input, None, &config).unwrap();

        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        // Content should preserve #| lines
        assert!(blocks[0].source.contains("#| label: main"));
    }

    #[test]
    fn test_quarto_via_extension() {
        // .qmd files automatically use Quarto style
        let input = r#"
```{python}
#| label: main
print('hello')
```
"#;
        let mut config = config_with_style(Style::EntangledRs);
        config.strip_quarto_options = true;
        let path = Path::new("doc.qmd");
        let doc = parse_markdown(input, Some(path), &config).unwrap();

        // namespace_default is None, so no prefix
        let blocks = doc.refs.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].source, "print('hello')");
    }

    // Style is determined per-document, not per-block
    #[test]
    fn test_md_file_uses_config_default() {
        // .md files use the configured default style
        let input = r#"
```python #native-block
native style
```
"#;
        let mut config = Config::default();
        config.namespace_default = NamespaceDefault::None;
        config.style = Style::EntangledRs;
        let path = Path::new("doc.md");
        let doc = parse_markdown(input, Some(path), &config).unwrap();

        assert_eq!(doc.refs.len(), 1);
        assert!(doc.refs.contains_name(&ReferenceName::new("native-block")));
    }

    #[test]
    fn test_qmd_ignores_config_default() {
        // .qmd files use Quarto style regardless of config
        let input = r#"
```{python}
#| label: quarto-block
quarto style
```
"#;
        let mut config = Config::default();
        config.namespace_default = NamespaceDefault::None;
        config.style = Style::EntangledRs; // This is ignored for .qmd
        config.strip_quarto_options = true;
        let path = Path::new("doc.qmd");
        let doc = parse_markdown(input, Some(path), &config).unwrap();

        assert_eq!(doc.refs.len(), 1);
        assert!(doc.refs.contains_name(&ReferenceName::new("quarto-block")));
    }

    #[test]
    fn test_rmd_ignores_config_default() {
        // .Rmd files use Knitr style regardless of config
        let input = r#"
```{r, label=knitr-block}
knitr style
```
"#;
        let mut config = Config::default();
        config.namespace_default = NamespaceDefault::None;
        config.style = Style::EntangledRs; // This is ignored for .Rmd
        let path = Path::new("doc.Rmd");
        let doc = parse_markdown(input, Some(path), &config).unwrap();

        assert_eq!(doc.refs.len(), 1);
        assert!(doc.refs.contains_name(&ReferenceName::new("knitr-block")));
    }

    // Test file-only blocks (no explicit ID)
    #[test]
    fn test_pandoc_file_only() {
        let input = r#"
``` {.python file=out.py}
print('hello')
```
"#;
        let config = config_with_style(Style::Pandoc);
        let doc = parse_markdown(input, None, &config).unwrap();

        assert_eq!(doc.refs.len(), 1);
        let blocks = doc.refs.get_by_name(&ReferenceName::from_file_path("out.py"));
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn test_knitr_file_only() {
        let input = r#"
```{python, file=out.py}
print('hello')
```
"#;
        let config = config_with_style(Style::Knitr);
        let doc = parse_markdown(input, None, &config).unwrap();

        assert_eq!(doc.refs.len(), 1);
        let blocks = doc.refs.get_by_name(&ReferenceName::from_file_path("out.py"));
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn test_quarto_file_only() {
        let input = r#"
```{python}
#| file: out.py
print('hello')
```
"#;
        let config = config_with_style(Style::Quarto);
        let doc = parse_markdown(input, None, &config).unwrap();

        assert_eq!(doc.refs.len(), 1);
        let blocks = doc.refs.get_by_name(&ReferenceName::from_file_path("out.py"));
        assert_eq!(blocks.len(), 1);
    }
}
