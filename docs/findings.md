# Findings

These are evidence-weighted trust-boundary observations, not exploit instructions. See the [evidence model](evidence-model.md) for the labels.

## Finding — A client-reported location is not proof of physical presence

### Evidence level

**Observed**, with a **hypothetical** security consequence.

### Observation

The studied action flow included a location claim originating in the client-driven workflow.

### Interpretation

A client can present a value without that value independently proving where the person or device is.

### Security boundary

Client-controlled claim → provider/backend trust decision.

### What this proves

It proves that location was part of the client-visible action model and should be treated as an input claim.

### What this does not prove

It does not prove that DEYSİS lacks server-side validation, corroborating signals, or policy controls.

### Risk if relied upon incorrectly

Using the claim as sole evidence of physical presence can weaken attendance integrity and audit confidence.

### Defensive recommendation

Treat location as one signal. Add server-authoritative session state and, where appropriate, proximity, attestation, instructor confirmation, or anomaly analysis.

## Finding — Device identity proves key possession, not presence

### Evidence level

**Observed**, with an **inferred** trust-boundary implication.

### Observation

The client workflow used a persistent device identity and asymmetric signing concept.

### Interpretation

A valid signature demonstrates possession of the corresponding private key. It does not independently establish the expected person, hardware state, or physical location.

### Security boundary

Local device state → provider identity and presence decision.

### What this proves

It proves that device identity is a useful accountability and key-management concept.

### What this does not prove

It does not establish how strongly production binds a device to a user or what additional controls exist.

### Risk if relied upon incorrectly

Treating a portable key as proof of presence collapses authentication, device identity, and presence into one assumption.

### Defensive recommendation

Keep those concepts separate. Consider attestation only as one layer, with explicit privacy and platform-dependence trade-offs.

## Finding — Challenge freshness is not enough without context binding

### Evidence level

**Observed** challenge/nonce concept; **public mock** binding and replay behavior.

### Observation

The analysis identified challenge-like state in the action lifecycle. The public implementation binds its invented challenge server-side to user, device, session, action, and expiry. Its versioned, length-prefixed signature payload covers that complete request context plus the location claim.

### Interpretation

A fresh value can still be misapplied if it is accepted outside its intended context or more than once.

### Security boundary

Challenge issuance → signed action execution.

### What this proves

The mock rejects expiry, replay, invalid signatures, and context mismatches through executable tests.

### What this does not prove

The mock is not evidence that production uses the same bindings or validation policy.

### Risk if relied upon incorrectly

Weakly scoped challenges can create replay or cross-context acceptance risk.

### Defensive recommendation

Use short-lived, server-generated, single-use challenges and consume them atomically with the intended action.
