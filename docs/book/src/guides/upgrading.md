# Upgrading

Replace the two binaries and restart the server. The saved index is
versioned; what happens on the first start after an upgrade depends on the
change:

| Change | Effect on the first start |
|---|---|
| Any release | The index loads, is reconciled against the tree, and files changed while the server was down are re-read |
| Symbol or reference extraction changed (the fingerprint's extraction tag differs) | Symbols and references are re-extracted from file content once, then saved; the log says "Persisted symbols predate the current extractor". Tens of seconds for 60k files |
| On-disk format changed (new magic number) | The old file is rejected with a message saying it must be rebuilt, and a full build runs |
| Configuration rules changed (excludes, extensions, size cap, `.gitignore`) | Files that are no longer eligible are dropped during reconciliation |

Nothing needs to be done by hand in any of these cases. Releases note
format changes in the [changelog](../imported/changelog.md).

## Version policy

Releases follow semantic versioning on a `0.x` line: a minor bump may
change ranking, output formats or the index file; a patch bump fixes
things. Only the latest release receives fixes
([security policy](../imported/security.md)).

## Downgrading

An index written by a newer format is rejected by an older server, which
then rebuilds. Keep the old binary's `index_path` separate if you need to
switch back and forth.
