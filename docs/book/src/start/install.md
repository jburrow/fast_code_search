# Install

Two binaries: `fast_code_search_server` (the index and its APIs) and `fcs`
(the command-line client). They are built together and released together.

## Release archive

Download the archive for your platform from the
[latest release](https://github.com/jburrow/fast_code_search/releases/latest),
verify it, unpack it, and put both binaries on your `PATH`:

```bash
tar -xzf fast_code_search-v*-x86_64-unknown-linux-gnu.tar.gz
sha256sum -c fast_code_search-v*-x86_64-unknown-linux-gnu.tar.gz.sha256
mkdir -p ~/.local/bin && cp fast_code_search_server fcs ~/.local/bin/
```

| Platform | Archive |
|---|---|
| Linux x86_64, glibc | `…-x86_64-unknown-linux-gnu.tar.gz` |
| Linux x86_64, static | `…-x86_64-unknown-linux-musl.tar.gz` (any distribution, no glibc dependency) |
| macOS, Apple silicon | `…-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `…-x86_64-apple-darwin.tar.gz` |
| Windows x86_64 | `…-x86_64-pc-windows-msvc.zip` |

Each archive also contains `config.toml.example`, the startup unit files
under `deploy/`, and the CLI and startup guides. On macOS, a downloaded
binary may need `xattr -d com.apple.quarantine <file>` before it runs.

## From source

Requires Rust 1.89 or newer and the Protocol Buffers compiler (`protoc`,
from your package manager or
[the releases page](https://github.com/protocolbuffers/protobuf/releases)).

```bash
cargo install --git https://github.com/jburrow/fast_code_search --bins
```

or, from a checkout, `cargo build --release` and take
`target/release/fast_code_search_server` and `target/release/fcs`.

## Check

```bash
fast_code_search_server --version
fcs --version
```

Next: [First index, first search](first-search.md).
