# Fictional mock protocol

> This protocol was invented for this repository and is intentionally incompatible with the production DEYSİS protocol.

The provider exposes four conceptual operations through the Rust trait: authenticate, register a device, request a challenge, and submit an action.

Enrollment validates the P-256 public key and immutably binds one public key to one device ID and owning user. Exact enrollment retries are idempotent; key replacement, ID rebinding, cross-user registration, and key aliasing under another ID are rejected.

The mock challenge is an opaque UUID stored server-side with these bindings: `user_id`, `device_id`, `session_id`, `action`, and `expires_at`. A submission includes those same context values, a coarse location claim, the challenge, and a URL-safe P-256 signature. The location claim is not stored in the challenge context; it is covered by the signature.

The signed bytes are a fictional, domain-separated binary payload:

```text
"deysis-trust-boundaries/action/v1\\0"
|| len(challenge) || challenge
|| len(user_id) || user_id
|| len(device_id) || device_id
|| len(session_id) || session_id
|| len(action) || action
|| len(location_claim) || location_claim
```

Lengths are unsigned 64-bit big-endian integers; the device ID is its 16 raw UUID bytes. This removes delimiter ambiguity and binds every request-context field. The mock rejects invalid signatures, unknown or expired challenges, replays, and binding mismatches. Validation and successful consumption happen under one lock; invalid attempts do not consume a legitimate holder's challenge. It returns a mock receipt on success. These names, fields, serialization, and semantics are not a description of the DEYSİS wire contract.
