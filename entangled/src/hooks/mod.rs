//! Hooks for extending Entangled functionality.

mod shebang;
mod spdx_license;

pub use shebang::ShebangHook;
pub use spdx_license::SpdxLicenseHook;

use crate::errors::Result;
use crate::model::CodeBlock;

/// A hook that can process code blocks.
pub trait Hook: Send + Sync {
    /// Returns the name of this hook.
    fn name(&self) -> &str;

    /// Processes a code block before tangling.
    ///
    /// Returns modified content if the hook made changes.
    fn pre_tangle(&self, block: &CodeBlock) -> Result<Option<PreTangleResult>>;

    /// Processes tangled output before writing.
    ///
    /// Returns modified content and optional prefix/suffix.
    fn post_tangle(&self, content: &str, block: &CodeBlock) -> Result<Option<PostTangleResult>>;
}

/// Result of pre-tangle hook processing.
#[derive(Debug, Clone)]
pub struct PreTangleResult {
    /// Modified source content.
    pub source: String,
    /// Metadata extracted by the hook.
    pub metadata: Vec<(String, String)>,
}

/// Result of post-tangle hook processing.
#[derive(Debug, Clone)]
pub struct PostTangleResult {
    /// Content to prepend to the output.
    pub prefix: Option<String>,
    /// Modified main content.
    pub content: String,
    /// Content to append to the output.
    pub suffix: Option<String>,
}

/// Registry of hooks.
#[derive(Default)]
pub struct HookRegistry {
    hooks: Vec<Box<dyn Hook>>,
}

impl std::fmt::Debug for HookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.hooks.iter().map(|h| h.name()).collect();
        f.debug_struct("HookRegistry")
            .field("hooks", &names)
            .finish()
    }
}

impl HookRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Adds a hook to the registry.
    pub fn add<H: Hook + 'static>(&mut self, hook: H) {
        self.hooks.push(Box::new(hook));
    }

    /// Returns the number of registered hooks.
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// Returns true if no hooks are registered.
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// Runs all pre-tangle hooks on a block.
    pub fn run_pre_tangle(&self, block: &CodeBlock) -> Result<Vec<PreTangleResult>> {
        let mut results = Vec::new();
        for hook in &self.hooks {
            if let Some(result) = hook.pre_tangle(block)? {
                results.push(result);
            }
        }
        Ok(results)
    }

    /// Runs all post-tangle hooks on content.
    pub fn run_post_tangle(&self, content: &str, block: &CodeBlock) -> Result<String> {
        let mut current = content.to_string();
        let mut prefix_parts = Vec::new();
        let mut suffix_parts = Vec::new();

        for hook in &self.hooks {
            if let Some(result) = hook.post_tangle(&current, block)? {
                if let Some(p) = result.prefix {
                    prefix_parts.push(p);
                }
                current = result.content;
                if let Some(s) = result.suffix {
                    suffix_parts.push(s);
                }
            }
        }

        let mut final_content = String::new();
        for p in prefix_parts {
            final_content.push_str(&p);
            final_content.push('\n');
        }
        final_content.push_str(&current);
        for s in suffix_parts {
            final_content.push('\n');
            final_content.push_str(&s);
        }

        Ok(final_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;

    struct TestHook {
        prefix: String,
    }

    impl Hook for TestHook {
        fn name(&self) -> &str {
            "test"
        }

        fn pre_tangle(&self, _block: &CodeBlock) -> Result<Option<PreTangleResult>> {
            Ok(None)
        }

        fn post_tangle(&self, content: &str, _block: &CodeBlock) -> Result<Option<PostTangleResult>> {
            Ok(Some(PostTangleResult {
                prefix: Some(self.prefix.clone()),
                content: content.to_string(),
                suffix: None,
            }))
        }
    }

    #[test]
    fn test_registry() {
        let mut registry = HookRegistry::new();
        assert!(registry.is_empty());

        registry.add(TestHook {
            prefix: "# Header".to_string(),
        });
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_post_tangle() {
        let mut registry = HookRegistry::new();
        registry.add(TestHook {
            prefix: "#!/usr/bin/env python".to_string(),
        });

        let block = test_utils::make_block("test", "code");
        let result = registry.run_post_tangle("print('hello')", &block).unwrap();

        assert!(result.starts_with("#!/usr/bin/env python\n"));
        assert!(result.contains("print('hello')"));
    }
}
