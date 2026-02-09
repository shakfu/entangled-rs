//! SPDX license header extraction hook.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::errors::Result;
use crate::model::CodeBlock;

use super::{Hook, PostTangleResult, PreTangleResult};

/// Pattern for SPDX license identifiers.
static SPDX_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?:#|//|--)\s*SPDX-License-Identifier:\s*(.+)$").unwrap());

/// Hook that extracts SPDX license headers from code blocks.
///
/// Recognizes SPDX-License-Identifier comments at the beginning of code blocks
/// and ensures they appear at the top of tangled output files.
#[derive(Debug, Clone, Default)]
pub struct SpdxLicenseHook;

impl SpdxLicenseHook {
    /// Creates a new SPDX license hook.
    pub fn new() -> Self {
        Self
    }

    /// Extracts SPDX license lines from the beginning of content.
    fn extract_spdx_lines(content: &str) -> Vec<String> {
        let mut spdx_lines = Vec::new();

        for line in content.lines() {
            if SPDX_PATTERN.is_match(line) {
                spdx_lines.push(line.to_string());
            } else if !line.trim().is_empty() {
                // Stop at first non-empty, non-SPDX line
                break;
            }
        }

        spdx_lines
    }

    /// Removes SPDX lines from the beginning of content.
    fn remove_spdx_prefix(content: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut skip = 0;

        for line in &lines {
            if SPDX_PATTERN.is_match(line) || line.trim().is_empty() {
                skip += 1;
            } else {
                break;
            }
        }

        lines[skip..].join("\n")
    }
}

impl Hook for SpdxLicenseHook {
    fn name(&self) -> &str {
        "spdx_license"
    }

    fn pre_tangle(&self, block: &CodeBlock) -> Result<Option<PreTangleResult>> {
        let spdx_lines = Self::extract_spdx_lines(&block.source);

        if spdx_lines.is_empty() {
            return Ok(None);
        }

        let new_source = Self::remove_spdx_prefix(&block.source);
        let spdx_header = spdx_lines.join("\n");

        Ok(Some(PreTangleResult {
            source: new_source,
            metadata: vec![("spdx_header".to_string(), spdx_header)],
        }))
    }

    fn post_tangle(&self, content: &str, block: &CodeBlock) -> Result<Option<PostTangleResult>> {
        let spdx_lines = Self::extract_spdx_lines(&block.source);

        if spdx_lines.is_empty() || !block.has_target() {
            return Ok(None);
        }

        let spdx_header = spdx_lines.join("\n");

        Ok(Some(PostTangleResult {
            prefix: Some(spdx_header),
            content: content.to_string(),
            suffix: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use std::path::PathBuf;

    #[test]
    fn test_extract_spdx_lines() {
        let content = "// SPDX-License-Identifier: MIT\n\nfn main() {}";
        let lines = SpdxLicenseHook::extract_spdx_lines(content);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "// SPDX-License-Identifier: MIT");
    }

    #[test]
    fn test_extract_multiple_spdx_lines() {
        let content = "// SPDX-License-Identifier: MIT\n// SPDX-FileCopyrightText: 2024\ncode";
        let lines = SpdxLicenseHook::extract_spdx_lines(content);

        // Only matches SPDX-License-Identifier
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_no_spdx() {
        let content = "fn main() {}";
        let lines = SpdxLicenseHook::extract_spdx_lines(content);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_hash_comment_spdx() {
        let content = "# SPDX-License-Identifier: Apache-2.0\nprint('hello')";
        let lines = SpdxLicenseHook::extract_spdx_lines(content);

        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Apache-2.0"));
    }

    #[test]
    fn test_pre_tangle() {
        let hook = SpdxLicenseHook::new();
        let block = test_utils::make_block_lang(
            "test",
            "// SPDX-License-Identifier: MIT\n\nfn main() {}",
            "rust",
        );

        let result = hook.pre_tangle(&block).unwrap().unwrap();
        assert!(!result.source.contains("SPDX"));
        assert!(result.metadata[0].1.contains("MIT"));
    }

    #[test]
    fn test_post_tangle_with_target() {
        let hook = SpdxLicenseHook::new();
        let block = test_utils::make_block_lang(
            "test",
            "// SPDX-License-Identifier: MIT\nfn main() {}",
            "rust",
        )
        .with_target(PathBuf::from("lib.rs"));

        let result = hook.post_tangle("fn main() {}", &block).unwrap().unwrap();
        assert!(result
            .prefix
            .unwrap()
            .contains("SPDX-License-Identifier: MIT"));
    }

    #[test]
    fn test_post_tangle_without_target() {
        let hook = SpdxLicenseHook::new();
        let block = test_utils::make_block_lang(
            "test",
            "// SPDX-License-Identifier: MIT\nfn main() {}",
            "rust",
        );

        let result = hook.post_tangle("fn main() {}", &block).unwrap();
        assert!(result.is_none());
    }
}
