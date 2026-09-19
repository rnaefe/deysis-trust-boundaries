# Reverse-engineering methodology

The private research followed a controlled, evidence-weighted process:

1. Observe legitimate client state transitions and record only the minimum notes needed for analysis.
2. Compare normal flows to identify stable values, ephemeral values, authentication boundaries, and lifecycle changes.
3. Map session creation, device registration, challenge issuance, signing, and action submission as a state machine.
4. Separate observation, inference, hypothesis, and confirmed behavior. An absent client-side check is not proof that a server-side check is absent.
5. Validate hypotheses with controlled tests using authorized accounts and synthetic inputs.
6. Identify which values originate with the client and which are authoritative server state.
7. Remove operational details from the public artifact and publish only reusable architectural lessons.

No exact requests, raw captures, production endpoints, headers, identifiers, or bypass procedures are part of this repository. The mock protocol is an independent design used to demonstrate secure binding and replay resistance.
