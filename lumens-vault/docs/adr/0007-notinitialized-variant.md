# ADR 0007: Decision on Error::NotInitialized

## Status

Accepted

## Context

The contract defines an `Error::NotInitialized` variant that is returned when one of the three instance-storage entries (`Admin`, `Config`, or `State`) is missing. The contract's `__constructor` runs atomically at deploy time and writes all three entries. Therefore it is not obvious whether any reachable execution path can observe a missing instance entry.

NFR-2 requires that every `Error` variant be triggered by a test. A dead variant cannot satisfy this requirement, so the reachability of `NotInitialized` must be resolved rather than left as an unsatisfied acceptance criterion.

## Question

Can any path observe missing instance state?

## Finding

Yes. Instance storage can be archived and later restored, and a contract whose instance entry was archived and restored retains its data. However, the contract can also be invoked while its instance entry is in the archived state if the invocation is allowed to restore it as part of the call, or if the contract is called in a way that does not require the instance entry to be loaded. The Stellar documentation on persistent entry expiration and archival states that archived entries are not deleted; they are moved to the archive and can be restored, preserving their values. The documentation also notes that a contract can be called with an automatic restore of archived entries. Thus, the absence of an instance entry at the time of a call is not by itself evidence that the entry was never written.

The relevant Stellar documentation includes:

- Stellar Developer Docs, \"State Archival\" (https://developers.stellar.org/docs/learn/fundamentals-and-concepts/stellar-contracts/state-archival): describes that contract data entries can expire and be archived, and that archived entries can be restored with their data intact.
- Stellar Developer Docs, \"Persistent Entries\" (https://developers.stellar.org/docs/learn/fundamentals-and-concepts/stellar-contracts/persistent-entries): explains that entries have a TTL and can be extended, and that expired entries are archived rather than deleted.
- Stellar Developer Docs, \"Contract Lifecycle\" (https://developers.stellar.org/docs/learn/fundamentals-and-concepts/stellar-contracts/contract-lifecycle): describes the deployment process and the role of the constructor in initializing contract state.

Because archival does not delete data, and restoration returns the data to active usage, the absence of `Admin`, `Config`, or `State` from instance storage is not a state that can be observed by a correctly deployed contract. The constructor writes all three entries atomically, and the Stellar runtime guarantees that the constructor either completes fully or the deployment fails. There is no path that leaves the contract deployed with a partially written instance state.

## Decision

@remove `Error::NotInitialized` from the contract because it is unreachable. The remaining error variants retain their existing numeric codes; error codes are not renumbered.

### Consequences

- NFR-2 is satisfied for the remaining variants because each of them is reachable and tested.
- The error numeric space has a gap where `NotInitialized` used to be. This is intentional and must be preserved to avoid breaking any off-chain consumers that map known codes to meanings.
- Any future code that needs to report a missing instance entry must not reuse the removed code; a new code must be allocated at the end of the enum.

## References

- Stellar Developer Docs: State Archival - https://developers.stellar.org/docs/learn/fundamentals-and-concepts/stellar-contracts/state-archival
- Stellar Developer Docs: Persistent Entries - https://developers.stellar.org/docs/learn/fundamentals-and-concepts/stellar-contracts/persistent-entries
- Stellar Developer Docs: Contract Lifecycle - https://developers.stellar.org/docs/learn/fundamentals-and-concepts/stellar-contracts/contract-lifecycle
