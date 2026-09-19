# Sanitized evidence ledger

This ledger makes the public reasoning traceable without publishing production endpoints, field names, values, captures, credentials, or a provider-compatible sequence. The IDs below are documentation IDs created for this repository, not identifiers from DEYSİS.

Public ledger last reviewed: **2026-09-17**. The exact provider build and original observation dates are not available in the public artifact, so freshness is **unknown**. This ledger records sanitized derived observations, not primary packet captures. Readers can audit how a conclusion follows from the public trace, but cannot independently reproduce the production observation from this repository alone.

## Field lifecycle summary

| Public alias | First visible phase | Stability within a legitimate flow | Classification | What it supports |
| --- | --- | --- | --- | --- |
| `account_ref` | Authentication | Stable | Observed | Actions occur in an authenticated account context |
| `session_ref` | Authentication result | Stable within a session; changes across sessions | Observed | Session state is distinct from long-lived identity |
| `device_ref` | Device lifecycle | Persists beyond one action flow | Observed | A client-visible device identity exists |
| `challenge_ref` | Pre-action phase | Ephemeral | Observed | The action lifecycle includes challenge-like state |
| `location_claim` | Action preparation | Supplied during the client flow | Observed | Location data crosses the client/server boundary |
| Exact server checks | Inaccessible backend | Not observable | Unknown | No conclusion about acceptance policy |

## Sanitized trace

```text
TRACE-01  unauthenticated -> authenticated(session_ref)
TRACE-02  authenticated -> device-ready(device_ref)
TRACE-03  device-ready -> challenge-issued(challenge_ref)
TRACE-04  challenge-issued -> action-submitted(location_claim, signing-related material)
TRACE-05  action-submitted -> result
```

This ordering is deliberately incomplete and non-operational. It records only the lifecycle distinctions used by the findings.

## OBS-SES-01 — session lifecycle

- Procedure: compare more than one legitimate authentication/action lifecycle at the state-transition level.
- Sanitized observation: an authenticated session reference appears before device/action phases and is not treated as the long-lived account identifier.
- Supports: authentication and session state are separate concepts.
- Does not support: session expiry, server revocation behavior, token format, or resistance to theft.

## OBS-DEV-01 — device lifecycle

- Procedure: compare the client-visible device state across legitimate action lifecycles.
- Sanitized observation: a device reference persists beyond a single action flow.
- Supports: the client participates in a persistent device-identity lifecycle.
- Does not support: hardware backing, exclusive ownership, attestation, or the server's enrollment checks.

## OBS-CHL-01 — challenge lifecycle

- Procedure: compare stable and changing values around the pre-action transition.
- Sanitized observation: challenge-like state is introduced after device readiness and before action submission.
- Supports: a challenge/nonce concept exists in the observed client lifecycle.
- Does not support: entropy, server generation, one-time consumption, expiry, or binding policy. Those are **public-mock** properties only.

## OBS-LOC-01 — location claim lifecycle

- Procedure: trace when location-related data enters the legitimate action flow.
- Sanitized observation: a location claim originates in the client-visible flow and crosses the trust boundary during action preparation/submission.
- Supports: location should be modeled as an untrusted client-originated claim.
- Does not support: whether the server corroborates it with independent signals or how it affects acceptance.

## Public audit boundary

The repository intentionally withholds the material needed to reproduce a production request. That safety choice limits independent verification. If stronger external auditability becomes possible without adding abuse value, the appropriate additions would be timestamped, redacted state-transition exports and hashes of retained source captures reviewed through an authorized disclosure channel—not production-compatible traffic samples.
