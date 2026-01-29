//! I/O operations for file handling and persistence.

mod file_cache;
mod filedb;
mod stat;
mod transaction;

pub use file_cache::{FileCache, RealFileCache, VirtualFS};
pub use filedb::FileDB;
pub use stat::{hexdigest_file, hexdigest_str, FileData, Stat};
pub use transaction::{Action, Create, Delete, Transaction, WriteAction};
