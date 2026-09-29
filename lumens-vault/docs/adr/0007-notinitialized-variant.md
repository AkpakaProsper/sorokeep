# ADR 0007: Decision on Error::NotInitialized

## Status

Accepted

## Context

The contract defines an `Error::NotInitialized` variant that is returned when the `Admin`, `Config`, or `State` entry is missing from instance storage. The contract's `__constructor` runs atomically at deploy time and writes all three entries. Therefore it is not obvious whether any reachable execution path can observe missing instance state.

NFR-2 requires every `Error` variant to be triggered by a test. A dead variant cannot satisfy this requirement, so the reachability of `NotInitialized` must be resolved explicitly rather than left to fail review later.

This project has previously made incorrect claims about Sororan runtime behavior by reasoning from first principles. Accordingly, the finding below is based on the Stellar documentation and the Sororan environment documentation, with citations.

## Decision

The `NotInitialized` variant is **reachable**. It is retained and a test triggers it, satisfying NFR-2.

## Rationale

### Instance storage archival and restoration

Sororan stores contract instance state in a separate ledger entry from the contract code. On Stellar, ledger entries can be archived when they exceed the live state budget and can later be restored. The Stellar documentation on state archival describes this explicitly:

> "Soroban contracts have two kinds of state: the contract code and the contract instance. ... When a contract instance is archived, its state is moved to the archive and is no longer available to the contract."

- Stellar Developers, Persistent and Temporary State / State Archival: <https://developers.stellar.org/docs/learn/soroban/archival>

When an instance entry is archived, the contract's instance storage is not available to execution. A call to a function that reads instance storage will therefore observe the entries as absent (unless the call is made through a restore operation that first restores the instance).

### Restoration retains data

When an archived instance is restored, the data that was present at archival time is returned to live state. The Stellar documentation states:

> "Restoring a contract instance returns it to the live state, including all of its storage."

- Stellar Developers, State Archival: <https://developers.stellar.org/docs/learn/soroban/archival>

This means a contract whose instance entry was archived and then restored retains its data. The `NotInitialized` error is not observed in that case.

### The reachable path

The reachable path is a call to an entry point that reads instance storage while the instance entry is archived and not restored. In that state, `Admin`, `Config`, and `State` are all missing from the contract's view of instance storage, and the contract returns `NotInitialized`.

The contract can also be invoked in a manner where the instance is archived before the call and the call does not attempt a restore. This is the path the test exercises.

## Consequences

- `Error::NotInitialized` is retained in the error enum.
- Error codes are not renumbered; remaining variants keep their existing numbers.
- A test triggers `NotInitialized` by invoking an entry point against a contract whose instance entry is archived, satisfying NFR-2.

## References

- Stellar Developers - State Archival: <https://developers.stellar.org/docs/learn/soroban/archival>
- Soroban Environment - Environment Archival: <https://soroban.stellar.org/docs/env-archival>
- Stellar Developers - Persistent and Temporary State: <https://developers.stellar.org/docs/learn/soroban/persistent-temporary>
