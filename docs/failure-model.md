# Failure model

| Failure | Detection | Behavior | Retry | Durable state |
| --- | --- | --- | --- | --- |
| PostgreSQL unavailable | Connection/query error | Composition must fail before accepting work | Host policy | Existing rows remain in PostgreSQL |
| Queue unavailable | Enqueue error | Do not report acceptance | Host policy | No false success record |
| Mock provider unavailable | `ProviderError` | Return failure; never convert to success | Caller policy | Job remains eligible for retry |
| Malformed job | Deserialization/validation | Reject before provider execution | No automatic retry | Rejection should be recorded |
| Invalid signature | Provider verification | Reject the action | No | Challenge is not accepted |
| Expired/replayed challenge | Provider state check | Reject the action | No | Challenge cannot be reused |
| Worker restart | Process exit | Queue lease/recovery is required | Queue policy | PostgreSQL rows are durable |
| Notification failure | Separate client result | Keep provider result distinct | Notification policy | Provider outcome remains authoritative |

The sample runner focuses on the provider and concurrency seams. The PostgreSQL adapter and schema show the persistence boundary, but this small repository does not implement a complete lease/retry loop or notification channel; those cells are explicit host-application responsibilities rather than claims about current code.
