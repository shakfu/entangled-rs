//! Python bindings for Entangled literate programming engine.

use std::path::PathBuf;

use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use entangled::Style;
use entangled::config::{self, AnnotationMethod, NamespaceDefault};
use entangled::interface::{self, Context, Document};
use entangled::io::Transaction;
use entangled::model::{CodeBlock, ReferenceMap, ReferenceName};

/// Convert entangled errors to Python exceptions.
fn to_py_err(e: entangled::errors::EntangledError) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// Python wrapper for Config.
#[pyclass(name = "Config")]
#[derive(Clone)]
pub struct PyConfig {
    inner: entangled::Config,
}

#[pymethods]
impl PyConfig {
    #[new]
    #[pyo3(signature = ())]
    fn new() -> Self {
        PyConfig {
            inner: entangled::Config::default(),
        }
    }

    /// Load configuration from a directory (looks for entangled.toml).
    #[staticmethod]
    fn from_dir(path: &str) -> PyResult<Self> {
        let config = config::read_config(&PathBuf::from(path)).unwrap_or_default();
        Ok(PyConfig { inner: config })
    }

    /// Load configuration from a specific file.
    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        let config = config::read_config_file(&PathBuf::from(path))
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(PyConfig { inner: config })
    }

    /// Get source patterns.
    #[getter]
    fn source_patterns(&self) -> Vec<String> {
        self.inner.source_patterns.clone()
    }

    /// Set source patterns.
    #[setter]
    fn set_source_patterns(&mut self, patterns: Vec<String>) {
        self.inner.source_patterns = patterns;
    }

    /// Get annotation method as string.
    #[getter]
    fn annotation(&self) -> String {
        match self.inner.annotation {
            AnnotationMethod::Standard => "standard".to_string(),
            AnnotationMethod::Naked => "naked".to_string(),
            AnnotationMethod::Supplemental => "supplemental".to_string(),
        }
    }

    /// Set annotation method from string.
    #[setter]
    fn set_annotation(&mut self, value: &str) -> PyResult<()> {
        self.inner.annotation = match value {
            "standard" => AnnotationMethod::Standard,
            "naked" => AnnotationMethod::Naked,
            "supplemental" => AnnotationMethod::Supplemental,
            _ => return Err(PyValueError::new_err("Invalid annotation method")),
        };
        Ok(())
    }

    /// Get namespace default as string.
    #[getter]
    fn namespace_default(&self) -> String {
        match self.inner.namespace_default {
            NamespaceDefault::File => "file".to_string(),
            NamespaceDefault::None => "none".to_string(),
        }
    }

    /// Set namespace default from string.
    #[setter]
    fn set_namespace_default(&mut self, value: &str) -> PyResult<()> {
        self.inner.namespace_default = match value {
            "file" => NamespaceDefault::File,
            "none" => NamespaceDefault::None,
            _ => return Err(PyValueError::new_err("Invalid namespace default")),
        };
        Ok(())
    }

    /// Get style as string.
    #[getter]
    fn style(&self) -> String {
        self.inner.style.name().to_string()
    }

    /// Set style from string.
    #[setter]
    fn set_style(&mut self, value: &str) -> PyResult<()> {
        self.inner.style = value.parse::<Style>().map_err(PyValueError::new_err)?;
        Ok(())
    }

    /// Get output directory.
    #[getter]
    fn output_dir(&self) -> Option<String> {
        self.inner
            .output_dir
            .as_ref()
            .map(|p| p.display().to_string())
    }

    /// Set output directory.
    #[setter]
    fn set_output_dir(&mut self, value: Option<String>) {
        self.inner.output_dir = value.map(PathBuf::from);
    }

    /// Get hooks.shebang setting.
    #[getter]
    fn hooks_shebang(&self) -> bool {
        self.inner.hooks.shebang
    }

    /// Set hooks.shebang setting.
    #[setter]
    fn set_hooks_shebang(&mut self, value: bool) {
        self.inner.hooks.shebang = value;
    }

    /// Get hooks.spdx_license setting.
    #[getter]
    fn hooks_spdx_license(&self) -> bool {
        self.inner.hooks.spdx_license
    }

    /// Set hooks.spdx_license setting.
    #[setter]
    fn set_hooks_spdx_license(&mut self, value: bool) {
        self.inner.hooks.spdx_license = value;
    }

    /// Get file database path.
    #[getter]
    fn filedb_path(&self) -> String {
        self.inner.filedb_path.display().to_string()
    }

    /// Set file database path.
    #[setter]
    fn set_filedb_path(&mut self, value: &str) {
        self.inner.filedb_path = PathBuf::from(value);
    }

    /// Get strip_quarto_options setting.
    #[getter]
    fn strip_quarto_options(&self) -> bool {
        self.inner.strip_quarto_options
    }

    /// Set strip_quarto_options setting.
    #[setter]
    fn set_strip_quarto_options(&mut self, value: bool) {
        self.inner.strip_quarto_options = value;
    }

    /// Get watch debounce delay in milliseconds.
    #[getter]
    fn watch_debounce_ms(&self) -> u64 {
        self.inner.watch.debounce_ms
    }

    /// Set watch debounce delay in milliseconds.
    #[setter]
    fn set_watch_debounce_ms(&mut self, value: u64) {
        self.inner.watch.debounce_ms = value;
    }

    fn __repr__(&self) -> String {
        format!(
            "Config(style='{}', annotation='{}', namespace_default='{}', source_patterns={:?})",
            self.style(),
            self.annotation(),
            self.namespace_default(),
            self.source_patterns()
        )
    }
}

/// Python wrapper for Transaction.
#[pyclass(name = "Transaction")]
pub struct PyTransaction {
    inner: Transaction,
}

#[pymethods]
impl PyTransaction {
    /// Check if transaction is empty.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get number of actions in transaction.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Get descriptions of all actions.
    fn describe(&self) -> Vec<String> {
        self.inner.describe()
    }

    /// Get unified diffs for all actions.
    fn diffs(&self) -> Vec<String> {
        self.inner.diffs()
    }

    fn __repr__(&self) -> String {
        format!("Transaction({} actions)", self.inner.len())
    }
}

/// Python wrapper for Context.
#[pyclass(name = "Context")]
pub struct PyContext {
    inner: Context,
}

#[pymethods]
impl PyContext {
    /// Create a new context with configuration and base directory.
    #[new]
    #[pyo3(signature = (config=None, base_dir=None))]
    fn new(config: Option<PyConfig>, base_dir: Option<&str>) -> PyResult<Self> {
        let cfg = config.map(|c| c.inner).unwrap_or_default();
        let dir = base_dir
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        let ctx = Context::new(cfg, dir).map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(PyContext { inner: ctx })
    }

    /// Create context from current directory.
    #[staticmethod]
    fn from_current_dir() -> PyResult<Self> {
        let ctx = Context::from_current_dir().map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(PyContext { inner: ctx })
    }

    /// Create context with default config for a specific directory.
    #[staticmethod]
    fn default_for_dir(path: &str) -> PyResult<Self> {
        let ctx = Context::default_for_dir(PathBuf::from(path))
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(PyContext { inner: ctx })
    }

    /// Get the base directory.
    #[getter]
    fn base_dir(&self) -> String {
        self.inner.base_dir.display().to_string()
    }

    /// Get source files matching the configuration patterns.
    fn source_files(&self) -> PyResult<Vec<String>> {
        let files = self.inner.source_files().map_err(to_py_err)?;
        Ok(files.into_iter().map(|p| p.display().to_string()).collect())
    }

    /// Resolve a relative path against the base directory.
    fn resolve_path(&self, path: &str) -> String {
        self.inner
            .resolve_path(&PathBuf::from(path))
            .display()
            .to_string()
    }

    /// Save the file database.
    fn save_filedb(&mut self) -> PyResult<()> {
        self.inner.save_filedb().map_err(to_py_err)
    }

    /// Get number of tracked files in the database.
    fn tracked_file_count(&self) -> usize {
        self.inner.filedb.len()
    }

    /// Get list of tracked files.
    fn tracked_files(&self) -> Vec<String> {
        self.inner
            .filedb
            .tracked_files()
            .map(|p| p.display().to_string())
            .collect()
    }

    /// Clear the file database.
    fn clear_filedb(&mut self) {
        self.inner.filedb.clear();
    }

    fn __repr__(&self) -> String {
        format!(
            "Context(base_dir='{}', tracked_files={})",
            self.base_dir(),
            self.tracked_file_count()
        )
    }
}

/// Python wrapper for CodeBlock.
#[pyclass(name = "CodeBlock")]
#[derive(Clone)]
pub struct PyCodeBlock {
    inner: CodeBlock,
}

#[pymethods]
impl PyCodeBlock {
    /// Get the block's reference ID as string.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }

    /// Get the block's reference name.
    #[getter]
    fn name(&self) -> String {
        self.inner.id.name.to_string()
    }

    /// Get the language identifier.
    #[getter]
    fn language(&self) -> Option<String> {
        self.inner.language.clone()
    }

    /// Get the source content.
    #[getter]
    fn source(&self) -> String {
        self.inner.source.clone()
    }

    /// Get the target file path if this is a file target.
    #[getter]
    fn target(&self) -> Option<String> {
        self.inner.target.as_ref().map(|p| p.display().to_string())
    }

    /// Check if block is empty.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get number of lines in the block.
    fn line_count(&self) -> usize {
        self.inner.line_count()
    }

    fn __repr__(&self) -> String {
        let target_info = self
            .target()
            .map(|t| format!(", target='{}'", t))
            .unwrap_or_default();
        format!(
            "CodeBlock(id='{}', language={:?}, lines={}{})",
            self.id(),
            self.language(),
            self.line_count(),
            target_info
        )
    }
}

/// Python wrapper for Document.
#[pyclass(name = "Document")]
pub struct PyDocument {
    #[pyo3(get)]
    path: Option<String>,
    refs: ReferenceMap,
}

#[pymethods]
impl PyDocument {
    /// Load a document from a file.
    #[staticmethod]
    fn load(path: &str, ctx: &PyContext) -> PyResult<Self> {
        let doc = Document::load(&PathBuf::from(path), &ctx.inner).map_err(to_py_err)?;
        Ok(PyDocument {
            path: Some(path.to_string()),
            refs: doc.refs().clone(),
        })
    }

    /// Parse markdown content directly.
    #[staticmethod]
    #[pyo3(signature = (content, path=None, config=None))]
    fn parse(content: &str, path: Option<&str>, config: Option<PyConfig>) -> PyResult<Self> {
        let cfg = config.map(|c| c.inner).unwrap_or_default();
        let doc = entangled::readers::parse_markdown(content, path.map(std::path::Path::new), &cfg)
            .map_err(to_py_err)?;
        Ok(PyDocument {
            path: path.map(String::from),
            refs: doc.refs,
        })
    }

    /// Get all code blocks.
    fn blocks(&self) -> Vec<PyCodeBlock> {
        self.refs
            .blocks()
            .map(|b| PyCodeBlock { inner: b.clone() })
            .collect()
    }

    /// Get blocks by name.
    fn get_by_name(&self, name: &str) -> Vec<PyCodeBlock> {
        self.refs
            .get_by_name(&ReferenceName::new(name))
            .into_iter()
            .map(|b| PyCodeBlock { inner: b.clone() })
            .collect()
    }

    /// Get all target file paths.
    fn targets(&self) -> Vec<String> {
        self.refs
            .targets()
            .map(|p| p.display().to_string())
            .collect()
    }

    /// Get number of code blocks.
    fn __len__(&self) -> usize {
        self.refs.len()
    }

    fn __repr__(&self) -> String {
        let path_info = self
            .path
            .as_ref()
            .map(|p| format!("path='{}', ", p))
            .unwrap_or_default();
        format!(
            "Document({}blocks={}, targets={})",
            path_info,
            self.refs.len(),
            self.targets().len()
        )
    }
}

/// Tangle all documents in the context.
///
/// Returns a Transaction that can be inspected or executed.
#[pyfunction]
fn tangle_documents(ctx: &mut PyContext) -> PyResult<PyTransaction> {
    let tx = interface::tangle_documents(&ctx.inner).map_err(to_py_err)?;
    Ok(PyTransaction { inner: tx })
}

/// Tangle specific source files.
///
/// Returns a Transaction that can be inspected or executed.
#[pyfunction]
fn tangle_files(ctx: &mut PyContext, source_files: Vec<String>) -> PyResult<PyTransaction> {
    let paths: Vec<PathBuf> = source_files.into_iter().map(PathBuf::from).collect();
    let tx = interface::tangle_files(&ctx.inner, &paths).map_err(to_py_err)?;
    Ok(PyTransaction { inner: tx })
}

/// Stitch all documents in the context.
///
/// Returns a Transaction that can be inspected or executed.
#[pyfunction]
fn stitch_documents(ctx: &mut PyContext) -> PyResult<PyTransaction> {
    let tx = interface::stitch_documents(&ctx.inner).map_err(to_py_err)?;
    Ok(PyTransaction { inner: tx })
}

/// Stitch specific source files.
///
/// Returns a Transaction that can be inspected or executed.
#[pyfunction]
fn stitch_files(ctx: &mut PyContext, source_files: Vec<String>) -> PyResult<PyTransaction> {
    let paths: Vec<PathBuf> = source_files.into_iter().map(PathBuf::from).collect();
    let tx = interface::stitch_files(&ctx.inner, &paths).map_err(to_py_err)?;
    Ok(PyTransaction { inner: tx })
}

/// Execute a transaction.
#[pyfunction]
#[pyo3(signature = (transaction, ctx, force=false))]
fn execute_transaction(
    transaction: &PyTransaction,
    ctx: &mut PyContext,
    force: bool,
) -> PyResult<()> {
    if force {
        transaction
            .inner
            .execute_force(&mut ctx.inner.filedb)
            .map_err(to_py_err)?;
    } else {
        transaction
            .inner
            .execute(&mut ctx.inner.filedb)
            .map_err(to_py_err)?;
    }
    Ok(())
}

/// Synchronize all documents (stitch then tangle).
#[pyfunction]
#[pyo3(signature = (ctx, force=false))]
fn sync_documents(ctx: &mut PyContext, force: bool) -> PyResult<()> {
    interface::sync_documents(&mut ctx.inner, force).map_err(to_py_err)
}

/// Locate the markdown source for a line in a tangled file.
///
/// Returns a dict with keys "source_file", "source_line", "block_id",
/// or None if the line is an annotation marker.
#[pyfunction]
fn locate_source<'py>(
    py: Python<'py>,
    ctx: &PyContext,
    target_file: &str,
    target_line: usize,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let result = interface::locate_source(&ctx.inner, &PathBuf::from(target_file), target_line)
        .map_err(to_py_err)?;

    match result {
        Some(loc) => {
            let dict = PyDict::new(py);
            dict.set_item("source_file", loc.source_file.display().to_string())?;
            dict.set_item("source_line", loc.source_line)?;
            dict.set_item("block_id", loc.block_id.to_string())?;
            Ok(Some(dict))
        }
        None => Ok(None),
    }
}

/// Tangle a reference by name from a reference map.
#[pyfunction]
#[pyo3(signature = (doc, name, annotate=true))]
fn tangle_ref(doc: &PyDocument, name: &str, annotate: bool) -> PyResult<String> {
    let ref_name = ReferenceName::new(name);

    let (comment, markers) = if annotate {
        // Try to get comment style from first block with this name
        let comment = doc
            .refs
            .get_by_name(&ref_name)
            .first()
            .and_then(|b| b.language.as_ref())
            .and_then(|lang| {
                let config = entangled::Config::default();
                config.find_language(lang).map(|l| l.comment.clone())
            });
        let markers = Some(entangled::config::Markers::default());
        (comment, markers)
    } else {
        (None, None)
    };

    let result =
        entangled::model::tangle_ref(&doc.refs, &ref_name, comment.as_ref(), markers.as_ref())
            .map_err(to_py_err)?;

    Ok(result)
}

/// Python module definition.
#[pymodule]
mod _core {
    #[pymodule_export]
    use super::PyConfig as Config;

    #[pymodule_export]
    use super::PyContext as Context;

    #[pymodule_export]
    use super::PyTransaction as Transaction;

    #[pymodule_export]
    use super::PyCodeBlock as CodeBlock;

    #[pymodule_export]
    use super::PyDocument as Document;

    #[pymodule_export]
    use super::tangle_documents;

    #[pymodule_export]
    use super::tangle_files;

    #[pymodule_export]
    use super::stitch_documents;

    #[pymodule_export]
    use super::stitch_files;

    #[pymodule_export]
    use super::execute_transaction;

    #[pymodule_export]
    use super::sync_documents;

    #[pymodule_export]
    use super::locate_source;

    #[pymodule_export]
    use super::tangle_ref;
}
