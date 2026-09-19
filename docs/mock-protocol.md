# Fictional mock protocol

> This protocol was invented for this repository and is intentionally incompatible with the production DEYSİS protocol.

The provider exposes four conceptual operations through the Rust trait: authenticate, register a device, request a challenge, and submit an action.

The mock challenge is an opaque UUID stored server-side with these bindings: `user_id`, `device_id`, `session_id`, `action`, and `expires_at`. A submission includes those same context values, a coarse location claim, the challenge, and a URL-safe P-256 signature.

The signed message is the fictional string:

```text
challenge | session_id | action | location_claim
```

The mock rejects invalid signatures, unknown or expired challenges, replays, and binding mismatches. It returns a mock receipt on success. These names, fields, serialization, and semantics are not a description of the DEYSİS wire contract.
