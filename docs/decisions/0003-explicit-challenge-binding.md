# Decision: bind challenges to action context

## Context

A nonce that is merely fresh can still be misapplied if it is accepted in another session or for another action. The important defensive property is context, not just randomness.

## Decision

The mock stores user, device, session, action, and expiry with each challenge. Submission checks all bindings, verifies the signature over the action context, and consumes the challenge once.

## Alternatives considered

- Bind only to a session: insufficient when a session can perform multiple actions.
- Put all context in an unsigned client payload: easy to inspect but not integrity-protected.
- Use a stateless token: possible, but atomic one-time consumption would need a separate replay store.

## Consequences

Replay and context confusion are directly testable. The server retains short-lived challenge state, and clock/cleanup policy becomes part of the provider design.
