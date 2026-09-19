# Decision: PostgreSQL leases with at-least-once delivery

## Context

A durable row alone is not a queue. Multiple workers need exclusive claims, crashed work must become recoverable, and retries must terminate. External provider actions also cannot be made atomic with a local PostgreSQL transaction.

## Decision

Workers claim ready rows inside a transaction with `FOR UPDATE SKIP LOCKED`. Each claim increments the attempt count and receives a random lease token plus an expiry. Completion, renewal, and retry are conditional on the current token. Failures use capped exponential backoff; reaching `max_attempts` moves the row to `failed`. A recovery pass requeues expired leases or terminally fails exhausted work.

The system explicitly promises **at-least-once** execution. Provider integrations must supply an idempotency key or reconciliation mechanism for the crash window between external success and local settlement.

## Alternatives

- Status-only rows: rejected because stale workers and concurrent claimers cannot prove ownership.
- Delete-on-claim: rejected because a worker crash loses work.
- Exactly-once claim: rejected as an inaccurate promise across the PostgreSQL/provider boundary.

## Consequences

Queue ownership, recovery, retry, and terminal failure are executable and integration-tested. The host must schedule recovery and heartbeat long-running work. Duplicate external effects remain possible unless the provider boundary handles idempotency.
