# Findings

These findings distinguish observed client behavior from server-side conclusions. They are not exploit instructions.

## Client-reported location is not equivalent to physical presence

### Observation

The studied workflow included a location claim in the client-driven action flow. A client can generally control the value it presents.

### Security implication

A location value proves at most that a value was submitted. It is not, by itself, proof that the person or device was physically present.

### What this does not prove

The observation does not prove that every server-side validation or secondary signal is absent. The public study does not claim a confirmed production weakness beyond the trust assumption.

### Potential impact

If relied on as the sole presence signal, the claim can reduce attendance integrity and weaken audit confidence.

### Recommended mitigation

Treat location as one signal. Bind action state server-side and corroborate with short-lived challenges, platform signals, proximity, instructor confirmation, or anomaly detection as appropriate.

## Device identity is not physical presence

### Observation

The client workflow used a persistent device identity and asymmetric signing concept.

### Security implication

Proof that a key can sign proves possession of that key, not that the expected device, person, or physical location is present.

### What this does not prove

It does not establish that the provider accepts every device claim or lacks additional controls.

### Recommended mitigation

Use device identity as an accountability and key-management signal, not as a standalone presence proof. Consider attestation with explicit privacy and platform-dependence trade-offs.

## Challenge binding determines replay resistance

### Observation

The analysis identified challenge/nonce concepts in the action lifecycle. The public mock makes binding explicit and testable.

### Security implication

Challenges that are not scoped to the user, device, session, action, and expiry can be replayed or moved across contexts.

### Recommended mitigation

Use server-generated, short-lived, single-use challenges and consume them atomically with the action.
