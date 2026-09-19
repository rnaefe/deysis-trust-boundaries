# Evidence model

This repository separates what was seen from what was designed for the public reproduction.

| Label | Meaning |
| --- | --- |
| **Observed** | Directly visible during legitimate client operation. |
| **Inferred** | Strongly suggested by multiple observations, but not directly proven. |
| **Hypothetical** | A possible consequence if an additional server-side condition holds. |
| **Public mock** | Behavior implemented only here to demonstrate a safer design. |
| **Withheld** | Confirmed implementation detail omitted because it would add operational abuse value. |
| **Unknown** | Not externally distinguishable without backend access or additional authorized evidence. |

## Evidence matrix

| Concept | Classification | Evidence basis | Public treatment |
| --- | --- | --- | --- |
| Persistent device identity | Observed | Client lifecycle analysis | Reproduced generically with P-256 |
| Client-provided location claim | Observed | Action-flow analysis | Represented as a fictional coarse claim |
| Authentication/session lifecycle | Observed | Legitimate login and action sequencing | Reproduced behind a trait |
| Challenge/nonce concept | Observed | Action-flow analysis | Reproduced with stricter mock binding; location is signed separately |
| Exact server validation | Unknown | Backend was not available for inspection | No claim |
| Replay impact in production | Hypothetical | Depends on server-side binding and consumption | Discussed defensively only |
| Replay-safe challenge binding | Public mock | Independent defensive design | Fully tested |
| Exact request serialization | Withheld | Private protocol analysis | Intentionally absent |
| Production provider adapter | Withheld | Operational integration detail | Not implemented |

The mock's stricter behavior should not be read backward as evidence about DEYSİS. It is a safe experiment that makes a security property executable.
