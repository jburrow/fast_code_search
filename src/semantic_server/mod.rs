//! gRPC server for semantic search

pub mod service;

pub use service::semantic_search::semantic_code_search_server::SemanticCodeSearchServer;
pub use service::SemanticSearchService;
