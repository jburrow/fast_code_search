# Glossary

| Term | Definition |
|------|------------|
| **Trigram** | A 3-character sequence extracted from text. For example, "function" produces trigrams: "fun", "unc", "nct", "cti", "tio", "ion". Used as an index key to quickly find candidate files containing a search term. |
| **Roaring Bitmap** | A compressed data structure for storing sets of integers efficiently. Used to store which document IDs contain each trigram. Supports fast set operations (intersection, union) for combining multiple trigram lookups. |
| **Memory-mapped file** | A file access technique where the OS maps file contents directly into virtual memory. Allows reading files without explicit I/O calls—the OS handles paging data in/out as needed. Reduces memory usage for large codebases. |
| **AST (Abstract Syntax Tree)** | A tree representation of source code structure. Used via tree-sitter to identify symbol definitions (functions, classes, methods, types, etc.) for smarter ranking. |
| **Symbol** | A named code element like a function, class, method, or variable. Extracted using tree-sitter parsing. Symbol matches are boosted 3x in search results. |
| **TF-IDF** | Term Frequency–Inverse Document Frequency. A text vectorization technique that weights words by how important they are to a document relative to the corpus. Used for semantic search embeddings. |
| **Embedding** | A vector (array of numbers) representing text in a high-dimensional space where similar meanings are close together. Enables "find similar code" queries. |
| **gRPC** | Google Remote Procedure Call. A high-performance protocol for server communication. Used for streaming search results to IDE clients. |
