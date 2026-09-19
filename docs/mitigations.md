# Defensive mitigations

- Generate short-lived challenges on the server and bind them to the authenticated user, registered device, attendance session, intended action, and expiry.
- Consume challenges atomically. Reject unknown, expired, already-consumed, or context-mismatched challenges.
- Keep authentication, device identity, and physical-presence evidence as separate concepts in both data models and audit records.
- Treat client location as a claim. Corroborate it with proximity mechanisms, platform attestation, instructor confirmation, or risk-based anomaly analysis.
- Add rate limits, duplicate detection, impossible-travel checks, and review queues for suspicious patterns.
- Use server-authoritative attendance-session state and immutable audit events with privacy-aware retention.
- Keep credentials out of queues and logs; retrieve them at execution time and encrypt private key material at rest.

Every control has a cost. Device attestation adds platform dependence and rollout complexity. Instructor confirmation adds friction. Location verification can be inaccurate indoors and can reveal sensitive data. Stronger auditability increases storage and governance requirements. A layered design is more realistic than a single perfect signal.
