pub mod lazy_file_store;
pub mod persistence;
pub mod trigram;

pub use lazy_file_store::{LazyFileStore, LazyMappedFile};
pub use persistence::{PersistedFileMetadata, PersistedIndex};
pub use trigram::{
    extract_trigrams, extract_unique_trigrams, extract_unique_trigrams_lowercase, Trigram,
    TrigramIndex,
};

// Re-export commonly used persistence types for benchmarks and tests
pub use persistence::{batch_check_files, FileStatus};
