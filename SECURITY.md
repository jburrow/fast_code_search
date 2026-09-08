# Security policy

## Threat model

fast_code_search is a developer tool that indexes source trees you point it
at. Its REST API returns the full contents of any indexed file and the gRPC
`Index` RPC re-indexes paths on request (restricted to the configured
`indexer.paths`). There is **no authentication**, so:

- Both listeners bind to `127.0.0.1` by default. Keep them there on a
  developer machine.
- To share a server, put it behind a reverse proxy that authenticates (see
  [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)) and set explicit
  `server.cors_origins`; never bind `0.0.0.0` on a network you do not control.
- Every response carries `X-Content-Type-Options: nosniff`,
  `X-Frame-Options: DENY` and `Referrer-Policy: same-origin`; the web UI
  escapes all server-provided strings.
- Query parameters are bounded (page size, offset, context, timeout), every
  search runs under an engine deadline, and regexes are compiled with size
  and DFA limits, so a request cannot pin the server.

Release archives ship with SHA-256 checksums next to each asset.

## Supported versions

| Version | Supported |
|---------|-----------|
| Latest release (`0.x` line) | Yes |
| Older releases | No — upgrade to the latest release |

## Reporting a vulnerability

Please do not open a public issue. Use GitHub's private reporting:
**[Report a vulnerability](https://github.com/jburrow/fast_code_search/security/advisories/new)**.
If that is not possible, email the maintainer listed under `authors` in
`Cargo.toml`.

Include the version, a description of the impact, and a minimal reproduction
(a query, a request, or a file) if you have one.

What to expect:

- acknowledgement within 7 days;
- an assessment and, for confirmed issues, a fix or mitigation plan within
  30 days;
- credit in the changelog and release notes unless you prefer otherwise.

Fixes ship as a patch release of the latest version with a `[Security]`
entry in `CHANGELOG.md`.
