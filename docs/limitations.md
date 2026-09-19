# Limitations

- The analysis was based on observed client behavior, not DEYSİS backend source code.
- Some server-side controls cannot be observed externally.
- Failure to observe a validation does not prove that server validation is absent.
- Findings may become stale after provider updates.
- The mock implementation does not reproduce the real protocol.
- No claim is made that every DEYSİS workflow was inspected.
- The public code demonstrates architecture and defensive invariants, not production interoperability.
- The sanitized evidence ledger contains derived state transitions, not independently reproducible primary captures.
- The encrypted key store is process-local; the sample does not claim crash-durable key custody or hardware-backed keys.
- Queue delivery is at-least-once. External effects can repeat without provider-side idempotency or reconciliation.
- The in-memory mock does not implement rate limits or long-term cleanup for sessions, devices, and consumed challenges.
