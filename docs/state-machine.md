# Sanitized state machine

The analysis was organized as a lifecycle rather than as a list of requests. Labels show the relationship between research evidence and the public reproduction.

```mermaid
stateDiagram-v2
    [*] --> Unauthenticated
    Unauthenticated --> Authenticated: legitimate session created [OBSERVED]
    Authenticated --> DeviceReady: device identity available [OBSERVED]
    DeviceReady --> ChallengeIssued: challenge requested [OBSERVED / PUBLIC-MOCK]
    ChallengeIssued --> SignedAction: action context signed; location included [PUBLIC-MOCK]
    SignedAction --> Accepted: valid, fresh, bound [PUBLIC-MOCK]
    SignedAction --> Rejected: invalid, expired, replayed, or mismatched [PUBLIC-MOCK]
    ChallengeIssued --> Rejected: expiry [PUBLIC-MOCK]
```

The diagram intentionally omits production field names, endpoint ordering, serialization, and transport details. The existence of a conceptual transition does not prove the implementation or validation policy of every DEYSİS server path.
