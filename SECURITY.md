# Security

fast_code_search is a local developer tool. It has **no authentication**: the REST
API returns full file contents of everything it indexed and the gRPC `Index` RPC
indexes paths on request (restricted to the configured `indexer.paths`). Both
listeners therefore bind to `127.0.0.1` by default. If you bind to `0.0.0.0`, put
the server behind something that authenticates, and only list explicit
`server.cors_origins`.

## Reporting a vulnerability

Please do not open a public issue for a security problem. Email the maintainer
(see `authors` in `Cargo.toml`) with a description and, if possible, a minimal
reproduction. You will get an acknowledgement within a week.

## Supported versions

Only the latest release receives fixes.
