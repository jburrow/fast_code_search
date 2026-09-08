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

### Verifying a download

Every archive ships with a `.sha256` file and, from v0.13.0, a Sigstore
bundle (`.sigstore.json`) produced by the release workflow's own identity.
The checksum proves the file is intact; the signature proves it was built
by this repository's release workflow for this tag, and not merely
uploaded by someone holding the account.

```bash
sha256sum -c fast_code_search-v0.13.0-x86_64-unknown-linux-gnu.tar.gz.sha256
cosign verify-blob \
  --bundle fast_code_search-v0.13.0-x86_64-unknown-linux-gnu.tar.gz.sigstore.json \
  --certificate-identity-regexp '^https://github.com/jburrow/fast_code_search/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  fast_code_search-v0.13.0-x86_64-unknown-linux-gnu.tar.gz
```

[`cosign`](https://docs.sigstore.dev/cosign/installation/) is a single
binary; no key management is involved because the signing is keyless.

## With cargo

[`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) downloads
the release archive for your platform instead of compiling, using the
metadata in `Cargo.toml`. Until the crate is on crates.io, point it at the
repository:

```bash
cargo binstall --git https://github.com/jburrow/fast_code_search fast_code_search
```

Once `fast_code_search` is published on crates.io, `cargo binstall
fast_code_search` and `cargo install fast_code_search` work without the
`--git` flag.

## Container image

Every release from v0.13.0 is also published as
`ghcr.io/jburrow/fast_code_search` (tags `latest`, `0.13`, `0.13.0`). The
image indexes whatever is mounted at `/src`, watches it for changes, and
serves the web UI and REST API on port 8080 and gRPC on 50051:

```bash
docker run --rm -p 8080:8080 -v "$PWD:/src:ro" ghcr.io/jburrow/fast_code_search
```

To keep the index between runs mount a volume at
`/var/lib/fast_code_search`; to change any setting mount your own file at
`/etc/fast_code_search/config.toml` (the baked-in one is
[`deploy/docker/config.toml`](https://github.com/jburrow/fast_code_search/blob/main/deploy/docker/config.toml)).
The `fcs` client is in the image too: `docker exec <container> fcs 'fn main'`.

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
