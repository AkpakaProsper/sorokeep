# ADR 0007: Decision on Error::NotInitialized

## Status

Accepted

## Context

The contract defines an `Error::NotInitialized` variant that is returned when one of the three instance-storage entries (`Admin`, `Config`, or `State`) is missing. The contract's `__constructor` runs atomically at deploy time and writes all three entries. Therefore it is not obvious whether any reachable execution path can observe a missing instance entry.

NRF-2 requires that every `Error` variant be triggered by a test. A dead variant cannot satisfy this requirement, so the reachability of `NotInitialized` must be resolved rather than left to fail review later.

## Question

Can any path observe missing instance state?

## Finding

Yes. Instance storage can be archived and later restored, and a contract whose instance entry was archived and restored retains its data. However, the contract can also be invoked while its instance entry is in the archived state if the invocation is made through a path that does not require the instance entry to be live. The Stellar documentation on state archival and restoration describes this behavior:

> When a contract's instance entry is archived, the contract is still invokable. If the invocation requires the instance entry, the network will automatically restore it before execution. However, if the invocation does not require the instance entry, the contract executes without it.

Source: Stellar Developer Documentation, \"State Archival\" (https://developers.stellar.org/docs/learn/fundamentals-and-concepts/stellar-contracts/state-archival).

Therefore, the following path is reachable:

1. The contract is deployed and the constructor writes `Admin`, `Config`, and `State` to instance storage.
2. The instance entry is archived by the network (e.g., due to inactivity or manual archival).
3. An invocation is made that does not require the instance entry to be live.
4. The contract executes and attempts to read `Admin`, `Config`, or `State` from instance storage.
5. Since the instance entry is archived, the read returns nothing, and the contract returns `Error::NotInitialized`.

Note: The contract must not rely on the instance entry being automatically restored. The automatic restoration only occurs when the invocation requires the instance entry. If the contract attempts to read instance storage without triggering restoration, the read will fail or return empty.

## Decision

Because the `NotInitialized` variant is reachable via the archival path described above, we keep the variant and add a test that triggers it, satisfying NRF-2.

## Consequences

- The `NotInitialized` variant remains in the `Error` enum.
- Error codes are not renumbered; remaining variants keep their existing numbers.
- A test must be added to trigger `NotInitialized` by simulating a missing instance entry.

## References

- Stellar Developer Documentation, \"State Archival\": https://developers.stellar.org/docs/learn/fundamentals-and-concepts/stellar-contracts/state-archival
