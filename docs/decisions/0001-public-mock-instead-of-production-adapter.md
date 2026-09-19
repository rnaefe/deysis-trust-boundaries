# Decision: public mock instead of production adapter

## Context

The private research included enough client/server detail to make a compatible adapter operationally meaningful. The portfolio value, however, is in the boundary analysis and systems reasoning rather than in publishing a live integration.

## Decision

Expose an `AttendanceProvider` trait and implement only an invented in-memory provider. Document its protocol completely and state that it is intentionally incompatible with DEYSİS.

## Alternatives considered

- Publish the production adapter with redacted hostnames: rejected because endpoint and serialization fragments could still be combined into a working client.
- Remove the provider boundary entirely: rejected because it would hide the key architectural seam.
- Use a generic unrelated demo: rejected because it would lose the named-target research context.

## Consequences

Readers can inspect authentication, device registration, challenge binding, signatures, and failure semantics without access to a production integration. The mock cannot demonstrate undocumented production controls; that limitation is explicit.
