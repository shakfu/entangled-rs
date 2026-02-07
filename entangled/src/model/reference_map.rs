//! Reference map with dual-index for code block lookup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use indexmap::IndexMap;

use super::code_block::CodeBlock;
use super::reference_id::ReferenceId;
use super::reference_name::ReferenceName;
use crate::errors::{EntangledError, Result};

/// A map of code blocks with dual-index lookup.
///
/// - Primary index: `IndexMap<ReferenceId, Arc<CodeBlock>>` (preserves insertion order)
/// - Secondary index: `HashMap<ReferenceName, Vec<ReferenceId>>` (name lookup)
/// - Targets: `HashMap<PathBuf, ReferenceName>` (output file registry)
///
/// Blocks are stored behind `Arc` to allow cheap cloning when combining
/// reference maps from multiple documents during tangle.
#[derive(Debug, Clone, Default)]
pub struct ReferenceMap {
    /// Primary storage: ID -> CodeBlock (insertion order preserved).
    blocks: IndexMap<ReferenceId, Arc<CodeBlock>>,

    /// Name index: Name -> list of IDs with that name.
    name_index: HashMap<ReferenceName, Vec<ReferenceId>>,

    /// Target file registry: Path -> Reference name.
    targets: HashMap<PathBuf, ReferenceName>,

    /// Counter for generating unique IDs per name.
    counters: HashMap<ReferenceName, usize>,
}

impl ReferenceMap {
    /// Creates a new empty reference map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a code block, generating a unique ID.
    ///
    /// Returns the assigned ReferenceId.
    pub fn insert(&mut self, mut block: CodeBlock) -> ReferenceId {
        // Get or create counter for this name
        let count = self.counters.entry(block.name().clone()).or_insert(0);
        let id = ReferenceId::new(block.name().clone(), *count);
        *count += 1;

        // Update block's ID
        block.id = id.clone();

        // Register target if present
        if let Some(ref target) = block.target {
            self.targets.insert(target.clone(), block.name().clone());
        }

        // Update name index
        self.name_index
            .entry(block.name().clone())
            .or_default()
            .push(id.clone());

        // Insert into primary storage
        self.blocks.insert(id.clone(), Arc::new(block));

        id
    }

    /// Inserts a code block with a specific ID (for stitching).
    pub fn insert_with_id(&mut self, id: ReferenceId, block: CodeBlock) {
        self.insert_arc_with_id(id, Arc::new(block));
    }

    /// Inserts an `Arc<CodeBlock>` with a specific ID.
    ///
    /// This avoids deep-cloning when transferring blocks between maps.
    pub fn insert_arc_with_id(&mut self, id: ReferenceId, block: Arc<CodeBlock>) {
        // Update counter if necessary
        let count = self.counters.entry(id.name.clone()).or_insert(0);
        if id.count >= *count {
            *count = id.count + 1;
        }

        // Register target if present
        if let Some(ref target) = block.target {
            self.targets.insert(target.clone(), id.name.clone());
        }

        // Update name index
        self.name_index
            .entry(id.name.clone())
            .or_default()
            .push(id.clone());

        // Insert into primary storage
        self.blocks.insert(id, block);
    }

    /// Gets a code block by its ID.
    pub fn get(&self, id: &ReferenceId) -> Option<&CodeBlock> {
        self.blocks.get(id).map(|arc| arc.as_ref())
    }

    /// Gets all code blocks with the given name.
    pub fn get_by_name(&self, name: &ReferenceName) -> Vec<&CodeBlock> {
        self.name_index
            .get(name)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.blocks.get(id))
                    .map(|arc| arc.as_ref())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Gets all IDs for blocks with the given name.
    pub fn get_ids_by_name(&self, name: &ReferenceName) -> Vec<&ReferenceId> {
        self.name_index
            .get(name)
            .map(|ids| ids.iter().collect())
            .unwrap_or_default()
    }

    /// Gets the reference name for a target file.
    pub fn get_target_name(&self, path: &Path) -> Option<&ReferenceName> {
        self.targets.get(path)
    }

    /// Checks if a name exists in the map.
    pub fn contains_name(&self, name: &ReferenceName) -> bool {
        self.name_index.contains_key(name)
    }

    /// Checks if an ID exists in the map.
    pub fn contains_id(&self, id: &ReferenceId) -> bool {
        self.blocks.contains_key(id)
    }

    /// Returns all registered target files.
    pub fn targets(&self) -> impl Iterator<Item = &PathBuf> {
        self.targets.keys()
    }

    /// Returns all reference names.
    pub fn names(&self) -> impl Iterator<Item = &ReferenceName> {
        self.name_index.keys()
    }

    /// Returns all code blocks in insertion order.
    pub fn blocks(&self) -> impl Iterator<Item = &CodeBlock> {
        self.blocks.values().map(|arc| arc.as_ref())
    }

    /// Returns all (ID, CodeBlock) pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&ReferenceId, &CodeBlock)> {
        self.blocks.iter().map(|(id, arc)| (id, arc.as_ref()))
    }

    /// Returns all (ID, Arc<CodeBlock>) pairs in insertion order.
    ///
    /// Use this when transferring blocks between maps to avoid deep cloning.
    pub fn iter_arcs(&self) -> impl Iterator<Item = (&ReferenceId, &Arc<CodeBlock>)> {
        self.blocks.iter()
    }

    /// Returns the number of code blocks.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Returns true if there are no code blocks.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Concatenates all source code for blocks with the given name.
    pub fn concatenate_source(&self, name: &ReferenceName) -> Result<String> {
        let blocks = self.get_by_name(name);
        if blocks.is_empty() {
            return Err(EntangledError::ReferenceNotFound(name.clone()));
        }

        Ok(blocks
            .iter()
            .map(|b| b.source.as_str())
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// Returns the number of blocks with the given name.
    pub fn count_by_name(&self, name: &ReferenceName) -> usize {
        self.name_index.get(name).map(|v| v.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{make_block, make_block_with_target};

    #[test]
    fn test_insert_and_get() {
        let mut map = ReferenceMap::new();
        let block = make_block("main", "print('hello')");
        let id = map.insert(block);

        assert_eq!(id.name.as_str(), "main");
        assert_eq!(id.count, 0);

        let retrieved = map.get(&id).unwrap();
        assert_eq!(retrieved.source, "print('hello')");
    }

    #[test]
    fn test_multiple_blocks_same_name() {
        let mut map = ReferenceMap::new();

        let id1 = map.insert(make_block("main", "line1"));
        let id2 = map.insert(make_block("main", "line2"));
        let id3 = map.insert(make_block("main", "line3"));

        assert_eq!(id1.count, 0);
        assert_eq!(id2.count, 1);
        assert_eq!(id3.count, 2);

        let blocks = map.get_by_name(&ReferenceName::new("main"));
        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn test_get_by_name() {
        let mut map = ReferenceMap::new();
        map.insert(make_block("foo", "foo1"));
        map.insert(make_block("bar", "bar1"));
        map.insert(make_block("foo", "foo2"));

        let foo_blocks = map.get_by_name(&ReferenceName::new("foo"));
        assert_eq!(foo_blocks.len(), 2);

        let bar_blocks = map.get_by_name(&ReferenceName::new("bar"));
        assert_eq!(bar_blocks.len(), 1);
    }

    #[test]
    fn test_targets() {
        let mut map = ReferenceMap::new();
        map.insert(make_block_with_target("main", "code", "output.py"));

        let targets: Vec<_> = map.targets().collect();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], &PathBuf::from("output.py"));

        let name = map.get_target_name(&PathBuf::from("output.py")).unwrap();
        assert_eq!(name.as_str(), "main");
    }

    #[test]
    fn test_concatenate_source() {
        let mut map = ReferenceMap::new();
        map.insert(make_block("main", "line1"));
        map.insert(make_block("main", "line2"));
        map.insert(make_block("main", "line3"));

        let source = map.concatenate_source(&ReferenceName::new("main")).unwrap();
        assert_eq!(source, "line1\nline2\nline3");
    }

    #[test]
    fn test_concatenate_source_not_found() {
        let map = ReferenceMap::new();
        let result = map.concatenate_source(&ReferenceName::new("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn test_insertion_order() {
        let mut map = ReferenceMap::new();
        map.insert(make_block("c", "third"));
        map.insert(make_block("a", "first"));
        map.insert(make_block("b", "second"));

        let sources: Vec<_> = map.blocks().map(|b| b.source.as_str()).collect();
        assert_eq!(sources, vec!["third", "first", "second"]);
    }

    #[test]
    fn test_insert_with_id() {
        let mut map = ReferenceMap::new();

        let id = ReferenceId::new(ReferenceName::new("test"), 5);
        let block = make_block("test", "content");
        map.insert_with_id(id.clone(), block);

        assert!(map.contains_id(&id));

        // Next auto-generated ID should be 6
        let new_id = map.insert(make_block("test", "more"));
        assert_eq!(new_id.count, 6);
    }
}
