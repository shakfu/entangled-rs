//! Project status: how each generated file compares with what the sources say.
//!
//! This lives in the library rather than the CLI so that the native CLI, the
//! Python CLI and any embedding all report the *same* status values from the
//! same computation. Previously each front end derived its own answer and they
//! disagreed: the Python CLI could only list target paths.

use std::path::PathBuf;

use serde::Serialize;

use crate::errors::Result;
use crate::interface::{analyze_project, Context};
use crate::io::{FileDB, FileData};

/// The state of one generated file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileStatus {
    /// The file on disk matches what Entangled last wrote.
    UpToDate,
    /// The file has never been tangled, or is not tracked.
    NeedsTangle,
    /// The file was changed outside Entangled since it was last written.
    #[serde(rename = "modified")]
    ExternallyModified,
    /// Entangled has written this file before, but it is gone.
    Missing,
}

impl FileStatus {
    /// The stable machine-readable name, as used in JSON output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UpToDate => "up-to-date",
            Self::NeedsTangle => "needs-tangle",
            Self::ExternallyModified => "modified",
            Self::Missing => "missing",
        }
    }
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One generated file and its state.
#[derive(Debug, Clone, Serialize)]
pub struct TargetStatus {
    /// The target as written in the document (`file=` value).
    pub path: PathBuf,
    /// Where that target actually resolves, after `output_dir`.
    pub resolved_path: PathBuf,
    /// The file's state.
    pub status: FileStatus,
}

/// A whole project's status.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatus {
    /// Source documents matching the configured patterns.
    pub source_files: Vec<PathBuf>,
    /// Every generated file, in document order.
    pub targets: Vec<TargetStatus>,
    /// How many files the database tracks.
    pub tracked_count: usize,
}

impl ProjectStatus {
    /// Counts targets in the given state.
    pub fn count(&self, status: FileStatus) -> usize {
        self.targets.iter().filter(|t| t.status == status).count()
    }
}

/// Collects the status of every generated file in the project.
///
/// Errors from loading a document propagate: a document that cannot be parsed
/// means the reported status is incomplete, and silently reporting a partial
/// answer as if it were the whole picture is worse than failing.
pub fn collect_status(ctx: &Context) -> Result<ProjectStatus> {
    let source_files = ctx.source_files()?;
    let analysis = analyze_project(ctx)?;

    let mut targets = Vec::new();
    for target in analysis.refs.targets() {
        // The same resolver tangle writes through, so an `output_dir` project
        // does not report every target as missing.
        let resolved_path = ctx.resolve_target(target)?;
        let status = file_status(&resolved_path, &ctx.filedb)?;
        targets.push(TargetStatus {
            path: target.clone(),
            resolved_path,
            status,
        });
    }

    Ok(ProjectStatus {
        source_files,
        targets,
        tracked_count: ctx.filedb.len(),
    })
}

/// Determines the state of a single generated file.
fn file_status(path: &std::path::Path, filedb: &FileDB) -> Result<FileStatus> {
    if !path.exists() {
        return Ok(if filedb.is_tracked(path) {
            FileStatus::Missing
        } else {
            FileStatus::NeedsTangle
        });
    }

    let current = FileData::from_path(path)?;

    match filedb.get(path) {
        Some(recorded) if recorded.hexdigest == current.hexdigest => Ok(FileStatus::UpToDate),
        Some(_) => Ok(FileStatus::ExternallyModified),
        None => Ok(FileStatus::NeedsTangle),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, NamespaceDefault};
    use crate::interface::tangle_documents;

    fn project(doc: &str) -> (tempfile::TempDir, Context) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("doc.md"), doc).unwrap();
        let config = Config {
            namespace_default: NamespaceDefault::None,
            ..Default::default()
        };
        let ctx = Context::new(config, dir.path().to_path_buf()).unwrap();
        (dir, ctx)
    }

    #[test]
    fn an_untangled_target_needs_tangling() {
        let (_dir, ctx) = project("```python #main file=out.py\nprint(1)\n```\n");
        let status = collect_status(&ctx).unwrap();
        assert_eq!(status.targets.len(), 1);
        assert_eq!(status.targets[0].status, FileStatus::NeedsTangle);
    }

    #[test]
    fn a_freshly_tangled_target_is_up_to_date() {
        let (_dir, mut ctx) = project("```python #main file=out.py\nprint(1)\n```\n");
        tangle_documents(&ctx)
            .unwrap()
            .execute(&mut ctx.filedb)
            .unwrap();

        let status = collect_status(&ctx).unwrap();
        assert_eq!(status.targets[0].status, FileStatus::UpToDate);
        assert_eq!(status.count(FileStatus::UpToDate), 1);
    }

    #[test]
    fn an_edited_target_is_reported_as_modified() {
        let (dir, mut ctx) = project("```python #main file=out.py\nprint(1)\n```\n");
        tangle_documents(&ctx)
            .unwrap()
            .execute(&mut ctx.filedb)
            .unwrap();
        std::fs::write(dir.path().join("out.py"), "edited by hand\n").unwrap();

        let status = collect_status(&ctx).unwrap();
        assert_eq!(status.targets[0].status, FileStatus::ExternallyModified);
    }

    #[test]
    fn a_deleted_target_is_reported_as_missing() {
        let (dir, mut ctx) = project("```python #main file=out.py\nprint(1)\n```\n");
        tangle_documents(&ctx)
            .unwrap()
            .execute(&mut ctx.filedb)
            .unwrap();
        std::fs::remove_file(dir.path().join("out.py")).unwrap();

        let status = collect_status(&ctx).unwrap();
        assert_eq!(status.targets[0].status, FileStatus::Missing);
    }

    #[test]
    fn output_dir_is_applied_when_resolving_targets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("doc.md"),
            "```python #main file=out.py\nprint(1)\n```\n",
        )
        .unwrap();
        let config = Config {
            namespace_default: NamespaceDefault::None,
            output_dir: Some("generated".into()),
            ..Default::default()
        };
        let mut ctx = Context::new(config, dir.path().to_path_buf()).unwrap();
        tangle_documents(&ctx)
            .unwrap()
            .execute(&mut ctx.filedb)
            .unwrap();

        let status = collect_status(&ctx).unwrap();
        assert_eq!(status.targets[0].path, PathBuf::from("out.py"));
        assert!(status.targets[0]
            .resolved_path
            .ends_with("generated/out.py"));
        assert_eq!(status.targets[0].status, FileStatus::UpToDate);
    }
}
