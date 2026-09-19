# Architecture

```mermaid
flowchart LR
  Client[Untrusted client claims] --> Service[Application service]
  Service --> Queue[(Durable PostgreSQL queue)]
  Queue --> Worker[Bounded worker pool]
  Worker --> Provider[Provider abstraction]
  Provider --> Mock[In-memory mock provider]
  Worker --> Secrets[Credential lookup at execution time]
  Worker --> Keys[Encrypted private-key persistence]
```

The private system supplied the architectural evidence for the client, service, PostgreSQL persistence, durable Apalis queue, bounded worker lifecycle, device-key manager, provider client, and structured tracing. This repository preserves the reasoning and boundaries while replacing the external adapter with a fictional provider.

Trusted components are the application service, queue state, worker code, and server-side mock state. Client-submitted locations, action identifiers, and device claims are untrusted inputs. Persistent state includes queued work and encrypted key material; sessions and challenges are ephemeral. The provider boundary is a deliberate seam: application code cannot depend on production wire details.

Jobs carry identifiers and action metadata only. Passwords, bearer tokens, and private signing keys are retrieved or materialized at execution time and are never serialized into the queue payload.
