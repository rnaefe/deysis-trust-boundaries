# Decision: secrets are not serialized into jobs

## Context

Durable queues persist payloads and may expose them to operators, retries, or recovery tooling. Credentials and private keys have a longer security lifetime than an individual action.

## Decision

`Job` stores identifiers and action metadata only. Sensitive material is retrieved or materialized at execution time and is never part of the serialized payload.

## Alternatives considered

- Put credentials in the job for worker independence: rejected because queue persistence expands the secret exposure surface.
- Put encrypted credentials in the job: better than plaintext, but still duplicates sensitive material and complicates rotation.
- Require a separate secret service: viable at larger scale, but unnecessary for this small reproduction.

## Consequences

A host-level retry can use current credential state, and rotation does not require rewriting queued jobs. Workers need access to protected runtime state, and a user may fail at execution time even if enqueueing succeeded.
