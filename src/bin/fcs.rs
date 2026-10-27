//! `fcs` — search a fast_code_search index from the command line.
//!
//! Talks to a running server over its REST API; when none is reachable it
//! can search the on-disk index directly. See `docs/CLI.md`.

// mimalloc, not the system allocator: see `fast_code_search::heap_size`.
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let code = fast_code_search::cli::main_with_args(args);
    std::process::exit(code);
}
