# Failure model

| Failure | Detection | Behavior | Retry | Durable state |
| --- | --- | --- | --- | --- |
| PostgreSQL unavailable | Connection/query error | Composition must fail before accepting work | Host policy | Existing rows remain in PostgreSQL |
| Queue unavailable | Enqueue error | Do not report acceptance | Host policy | No false success record |
| Mock provider unavailable | `ProviderError` | Settle through retry policy | Exponential, capped | Row is requeued or terminally failed |
| Malformed job | Deserialization/validation | Reject before provider execution | No | Row is terminally failed with an error |
| Invalid signature | Provider verification | Reject the action | No | Challenge is not accepted |
| Expired/replayed challenge | Provider state check | Reject the action | No | Challenge cannot be reused |
| Worker restart | Lease expiry | `recover_expired_leases` requeues or terminally fails the row | Bounded by max attempts | PostgreSQL rows are durable |
| Crash after provider success | Missing queue settlement | Lease recovery can make the row runnable again | Possible duplicate without provider idempotency | At-least-once boundary is explicit |
| Notification failure | Separate client result | Keep provider result distinct | Notification policy | Provider outcome remains authoritative |

The sample implements one durable claim/execute/settle step and explicit expired-lease recovery. A host still owns scheduling the loop, periodic recovery, lease-heartbeat cadence for long actions, shutdown, metrics, notifications, and provider-side idempotency/reconciliation.
