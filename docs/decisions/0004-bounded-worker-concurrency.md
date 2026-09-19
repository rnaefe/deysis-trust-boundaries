# Decision: bound worker concurrency

## Context

Asynchronous jobs can outlive the client request. Unbounded fan-out makes provider load, database connections, and failure recovery unpredictable.

## Decision

The runner uses a Tokio semaphore to hold a permit for the complete provider interaction. The configured limit is tested with an instrumented provider.

## Alternatives considered

- Spawn one task per job: simple, but no backpressure.
- Use a fixed thread pool: mismatched with the I/O-bound async workflow.
- Rate-limit only at the provider adapter: too late to bound local work and resource use.

## Consequences

The system has a clear upper bound on in-flight provider work. Queue latency can increase under load, so capacity and retry policy should be measured for a real deployment.
