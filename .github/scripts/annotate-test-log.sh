#!/usr/bin/env bash
# Turn a `cargo test` log into GitHub workflow annotations: failed test
# names, panic messages, assertion details, and cargo's own "process
# didn't exit successfully" / signal lines. Falls back to the last 40 lines
# so a crash that printed none of those still leaves a trace.
log="$1"
found=0
while IFS= read -r line; do
  found=1
  printf '::error title=Failed test::%s\n' "$line"
done < <(grep -E '^test .* FAILED$' "$log" | sort -u)
while IFS= read -r line; do
  found=1
  printf '::error::%s\n' "$line"
done < <(grep -E -A2 "panicked at" "$log" | grep -v '^--$' | head -24; grep -E "assertion .* failed|^  left:|^  right:|didn't exit successfully|signal: |SIG[A-Z]+|error: test failed|error\[E[0-9]+\]|^error: could not compile" "$log" | head -16)
if [ "$found" -eq 0 ]; then
  printf '::error::no failure marker found in %s; last 40 lines follow\n' "$log"
  tail -40 "$log" | while IFS= read -r line; do printf '::error::%s\n' "$line"; done
fi
