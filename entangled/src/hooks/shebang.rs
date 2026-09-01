//! Shebang extraction hook.

use crate::errors::Result;
use crate::model::CodeBlock;

use super::{Hook, PostTangleResult, PreTangleResult};

/// Hook that extracts shebang lines from code blocks.
///
/// If the first line of a code block is a shebang (`#!...`),
/// it will be moved to the beginning of the tangled output.
#[derive(Debug, Clone, Default)]
pub struct ShebangHook;

impl ShebangHook {
    /// Creates a new shebang hook.
    pub fn new() -> Self {
        Self
    }

    /// Extracts a shebang line from content.
    fn extract_shebang(content: &str) -> Option<(&str, &str)> {
        let first_line = content.lines().next()?;
        if first_line.starts_with("#!") {
            let rest_start = first_line.len();
            let rest = if content.len() > rest_start {
                content[rest_start..].trim_start_matches('\n')
            } else {
                ""
            };
            Some((first_line, rest))
        } else {
            None
        }
    }
}

impl Hook for ShebangHook {
    fn name(&self) -> &str {
        "shebang"
    }

    fn pre_tangle(&self, block: &CodeBlock) -> Result<Option<PreTangleResult>> {
        if let Some((shebang, rest)) = Self::extract_shebang(&block.source) {
            Ok(Some(PreTangleResult {
                source: rest.to_string(),
                header: vec![shebang.to_string()],
                metadata: vec![("shebang".to_string(), shebang.to_string())],
            }))
        } else {
            Ok(None)
        }
    }

    fn post_tangle(&self, _content: &str, _block: &CodeBlock) -> Result<Option<PostTangleResult>> {
        // Nothing to do: the shebang is lifted out of the block by `pre_tangle`
        // and re-emitted once at the top of the target by the orchestrator.
        // Re-adding it here is what used to produce two shebangs per file.
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;

    #[test]
    fn test_extract_shebang() {
        let content = "#!/usr/bin/env python\nprint('hello')";
        let (shebang, rest) = ShebangHook::extract_shebang(content).unwrap();

        assert_eq!(shebang, "#!/usr/bin/env python");
        assert_eq!(rest, "print('hello')");
    }

    #[test]
    fn test_no_shebang() {
        let content = "print('hello')";
        assert!(ShebangHook::extract_shebang(content).is_none());
    }

    #[test]
    fn test_pre_tangle() {
        let hook = ShebangHook::new();
        let block = test_utils::make_block("test", "#!/bin/bash\necho hello");

        let result = hook.pre_tangle(&block).unwrap().unwrap();
        assert_eq!(result.source, "echo hello");
        assert_eq!(
            result.metadata[0],
            ("shebang".to_string(), "#!/bin/bash".to_string())
        );
    }

    #[test]
    fn test_pre_tangle_hoists_shebang_out_of_the_block() {
        let hook = ShebangHook::new();
        let block = test_utils::make_block_with_target(
            "test",
            "#!/usr/bin/env python\nprint('hello')",
            "script.py",
        );

        let result = hook.pre_tangle(&block).unwrap().unwrap();
        assert_eq!(result.header, vec!["#!/usr/bin/env python".to_string()]);
        // The shebang must be gone from the source, not merely copied out --
        // leaving it in both places is what produced two shebangs per file.
        assert_eq!(result.source, "print('hello')");
    }

    #[test]
    fn test_post_tangle_adds_nothing() {
        // The header is emitted once by the orchestrator from `pre_tangle`;
        // the post hook must not add a second copy.
        let hook = ShebangHook::new();
        let block = test_utils::make_block_with_target(
            "test",
            "#!/usr/bin/env python\nprint('hello')",
            "script.py",
        );

        assert!(hook
            .post_tangle("print('hello')", &block)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_post_tangle_without_target() {
        let hook = ShebangHook::new();
        let block = test_utils::make_block("test", "#!/usr/bin/env python\nprint('hello')");

        // No target, so shebang should not be added
        let result = hook.post_tangle("print('hello')", &block).unwrap();
        assert!(result.is_none());
    }
}
