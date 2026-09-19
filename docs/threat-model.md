# Threat model

## Assets

Attendance integrity, student identity, session state, device identity, location assertions, challenge state, audit records, and credentials.

## Attacker capability

The attacker controls their own client, can inspect their own client traffic, can modify their own software, can replay locally generated state, and can provide arbitrary client-controlled values. This model does not assume server compromise, administrator credentials, other users' accounts, or database compromise.

## Boundaries and questions

| Boundary | Security question |
| --- | --- |
| Client → backend | Which claims are treated as facts? |
| Device state → backend | Does identity prove presence or only key possession? |
| Location source → client | Is a location claim independently corroborated? |
| Challenge → action | Is the challenge scoped, fresh, expiring, and one-time? |
| Queue → worker | Can durable work omit secrets and survive restart? |
| Worker → provider | Are provider failures distinguishable from success? |

Risk classes include client-reported location, device identity assumptions, weak challenge binding, replay, long-lived credentials, insufficient anomaly detection, and weak auditability. Individual findings are labelled with their evidence level in [findings](findings.md).
