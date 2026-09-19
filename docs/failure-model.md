# Failure model

| Failure | Expected behavior |
| --- | --- |
| PostgreSQL unavailable | A real application composition should fail before accepting work. |
| Queue unavailable | Do not report work as accepted; retry or surface a durable error. |
| Mock provider unavailable | Return a provider error; never convert it into success. |
| Malformed job | Reject before provider execution. |
| Invalid signature | Provider rejects the action. |
| Expired or replayed challenge | Provider rejects the action. |
| Worker restart | Queued PostgreSQL rows remain available for recovery. |
| Notification failure | Keep provider outcome distinct from notification outcome. |

The sample runner focuses on the provider and concurrency seams. The PostgreSQL adapter and schema show the persistence boundary; production retry, leasing, and notification policy should be selected explicitly for the host application.
