pub mod service;

pub use service::{
    create_indexed_engine, create_server, create_server_with_engine, create_server_with_indexing,
    search_proto, CodeSearchService,
};
