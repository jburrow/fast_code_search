pub mod file_store;
pub mod persistence;
pub mod trigram;

pub use file_store::{FileStore, MappedFile};
pub use persistence::{PersistedFileMetadata, PersistedIndex};
pub use trigram::{extract_trigrams, extract_unique_trigrams, Trigram, TrigramIndex};

// Re-export commonly used persistence types for benchmarks and tests
pub use persistence::{batch_check_files, FileStatus};
