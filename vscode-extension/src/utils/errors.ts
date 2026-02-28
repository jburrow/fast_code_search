/** Returns true when an error represents a fetch/AbortController cancellation. */
export function isAbortError(err: unknown): boolean {
  return (
    err instanceof Error &&
    (err.name === "AbortError" || err.message.includes("abort"))
  );
}
