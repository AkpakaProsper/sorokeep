#![cfg(test)]
#![allow(deprecated)]

use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{
    testutils::{storage::Persistent, Address as _, Ledger, MockAuth, MockAuthInvoke},
    Address, BytesN, ConversionError, Env, IntoVal, InvokeError,
};

use crate::contract::Error;
use crate::storage::DataKey;
use crate::{LumensVault, LumensVaultClient};

// Build requirements:
//
// - Rust 1.85 or later. soroban-sdk 28's dependency tree needs edition2024,
//   and an older toolchain fails on a transitive dependency before reaching
//   this crate's own code.
// - The `wasm32v1-none` target, for `stellar contract build`.
//
// `test_real_upgrade_and_state_migration` needs the v2 fixture compiled
// first — see the comment above it for why and for the build order.

fn create_token_contract<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_address.address()),
        StellarAssetClient::new(env, &contract_address.address()),
    )
}

/// `env.register` now takes constructor args directly, since `initialize`
/// was replaced by `__constructor` (see contract.rs change log item 1).
fn setup(env: &Env, admin: &Address, default_timelock_ledgers: u32) -> LumensVaultClient<'static> {
    let vault_id = env.register(LumensVault, (admin, default_timelock_ledgers));
    LumensVaultClient::new(env, &vault_id)
}

#[test]
fn test_deposit_and_withdraw() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let vault_client = setup(&env, &admin, 10);

    let token_admin = Address::generate(&env);
    let (token_client, token_asset) = create_token_contract(&env, &token_admin);
    token_asset.mint(&user, &1000);

    vault_client.add_asset(&token_client.address);

    let returned_vault_id = vault_client.deposit(&user, &token_client.address, &100);
    assert_eq!(returned_vault_id, 1);

    assert_eq!(token_client.balance(&user), 900);
    assert_eq!(token_client.balance(&vault_client.address), 100);

    // Timelock not yet expired.
    let res = vault_client.try_withdraw(&user, &token_client.address, &1, &50);
    assert!(res.is_err());

    env.ledger().with_mut(|l| l.sequence_number += 11);

    vault_client.withdraw(&user, &token_client.address, &1, &50);

    assert_eq!(token_client.balance(&user), 950);
    assert_eq!(token_client.balance(&vault_client.address), 50);

    // New: the view function this pass added actually reflects the state.
    let entry = vault_client.get_vault(&user, &token_client.address, &1);
    assert_eq!(entry.amount, 50);
}

#[test]
fn test_deposit_rejects_non_positive_amount() {
    // NEW — covers the fix in contract.rs change log item 2. Before this
    // fix, neither of these guarded at all.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let vault_client = setup(&env, &admin, 10);

    let token_admin = Address::generate(&env);
    let (token_client, token_asset) = create_token_contract(&env, &token_admin);
    token_asset.mint(&user, &1000);
    vault_client.add_asset(&token_client.address);

    let zero_res = vault_client.try_deposit(&user, &token_client.address, &0);
    assert!(zero_res.is_err());

    let negative_res = vault_client.try_deposit(&user, &token_client.address, &-100);
    assert!(negative_res.is_err());
}

#[test]
fn test_withdraw_rejects_non_positive_amount() {
    // NEW — this is the more important half of the fix: without the guard,
    // a negative `amount` here would have skipped the insufficient-balance
    // check and *inflated* the caller's recorded balance via
    // `entry_v1.amount -= amount`. See contract.rs change log item 2 for
    // the full walkthrough.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let vault_client = setup(&env, &admin, 10);

    let token_admin = Address::generate(&env);
    let (token_client, token_asset) = create_token_contract(&env, &token_admin);
    token_asset.mint(&user, &1000);
    vault_client.add_asset(&token_client.address);

    vault_client.deposit(&user, &token_client.address, &500);
    env.ledger().with_mut(|l| l.sequence_number += 11);

    let res = vault_client.try_withdraw(&user, &token_client.address, &1, &-200);
    assert!(res.is_err());

    // Balance must be exactly what was deposited — not inflated.
    let entry = vault_client.get_vault(&user, &token_client.address, &1);
    assert_eq!(entry.amount, 500);
}

#[test]
fn test_user_vault_count_ttl_is_extended_on_deposit() {
    // NEW — covers contract.rs change log item 3. Before this fix,
    // `UserVaultCount` was written once on a user's first deposit and never
    // touched again, so it would archive on its own default schedule
    // regardless of how active the user was — silently blocking every
    // future deposit from that user once it did.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let vault_client = setup(&env, &admin, 10);

    let token_admin = Address::generate(&env);
    let (token_client, token_asset) = create_token_contract(&env, &token_admin);
    token_asset.mint(&user, &1000);
    vault_client.add_asset(&token_client.address);

    vault_client.deposit(&user, &token_client.address, &100);

    let count_key = DataKey::UserVaultCount(user.clone());
    let ttl_after_first_deposit =
        env.as_contract(&vault_client.address, || env.storage().persistent().get_ttl(&count_key));

    // Advance close to (but not past) the extension threshold and deposit
    // again — the TTL should be bumped back up, not left decaying.
    env.ledger()
        .with_mut(|l| l.sequence_number += ttl_after_first_deposit - 1000);

    vault_client.deposit(&user, &token_client.address, &50);

    let ttl_after_second_deposit =
        env.as_contract(&vault_client.address, || env.storage().persistent().get_ttl(&count_key));

    assert!(
        ttl_after_second_deposit > 1000,
        "UserVaultCount TTL was not refreshed on the second deposit — it would archive soon"
    );
}

// ---------------------------------------------------------------------
// The real upgrade test.
//
// This is a genuine cross-binary upgrade test, and the distinction matters:
// earlier versions of it wrote data through the V1 contract and read it back
// through the SAME running V1 binary, which proves storage round-trips and
// nothing about upgrades. Built the way Stellar's own docs build it:
// https://developers.stellar.org/docs/build/guides/conventions/upgrading-contracts
//
// It needs a second, genuinely separate crate — contracts/lumens-vault-v2-fixture/
// in the folder next to this one — compiled to wasm BEFORE this test runs,
// because `contractimport!` reads the compiled .wasm file at compile time,
// not at test time. Build order:
//
//   cd contracts/lumens-vault-v2-fixture && stellar contract build
//   cd ../lumens-vault && cargo test
//
// If your workspace layout puts these crates somewhere else, fix the path
// in the `contractimport!` call below to match.
// ---------------------------------------------------------------------

mod new_contract {
    soroban_sdk::contractimport!(
        file = "../lumens-vault-v2-fixture/target/wasm32v1-none/release/lumens_vault_v2_fixture.wasm"
    );
}

fn install_new_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(new_contract::WASM)
}

#[test]
fn test_real_upgrade_and_state_migration() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.sequence_number = 1000);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let vault_client = setup(&env, &admin, 10);

    let token_admin = Address::generate(&env);
    let (token_client, token_asset) = create_token_contract(&env, &token_admin);
    token_asset.mint(&user, &1000);
    vault_client.add_asset(&token_client.address);

    // 1. Write real state through the OLD contract's own deposit logic —
    //    not a raw storage poke.
    let returned_vault_id = vault_client.deposit(&user, &token_client.address, &500);
    assert_eq!(returned_vault_id, 1);
    assert_eq!(vault_client.version(), 1);

    // 2. Install a SECOND, genuinely different compiled binary and swap the
    //    SAME contract address over to it.
    let new_wasm_hash = install_new_wasm(&env);
    vault_client.upgrade(&new_wasm_hash);

    // 3. Prove the running bytecode actually changed. The V1 client type
    //    has no way to lie about this — `version()` only returns 2 if the
    //    call is genuinely being served by the new binary.
    assert_eq!(vault_client.version(), 2);

    // 4. The real claim: data written by the OLD binary as
    //    `VaultEntry::V1(..)` is read correctly by the NEW binary's own
    //    code, through a function (`get_vault` returning the V2 shape)
    //    that only exists post-upgrade. This has to go through a client
    //    typed against the NEW contract's interface — the old
    //    `LumensVaultClient` binding has no `get_vault` method to call.
    let new_client = new_contract::Client::new(&env, &vault_client.address);
    let migrated = new_client.get_vault(&user, &token_client.address, &1);

    assert_eq!(migrated.amount, 500);
    // `last_touched_ledger` only exists on VaultEntryV2 — its presence at
    // all is part of the proof that migration, not just a raw byte
    // round-trip, actually happened.
    assert!(migrated.last_touched_ledger > 0);
}

// ---------------------------------------------------------------------
// Authorization: the asset whitelist.
//
// Whitelisting decides which token contracts `deposit` will call `transfer`
// on, so anyone who can whitelist can point the vault at a contract of their
// choosing. These tests pin down that only the admin can.
//
// How a rejected `require_auth()` actually surfaces in soroban-sdk 28 — this
// is not what you would guess from the contract signature, and it is the
// usual source of confusion when writing auth tests here:
//
//   - A *contract* failure (`Err(Error::AssetNotWhitelisted)` and friends)
//     comes back from `try_*` as `Err(Ok(Error::..))`. The inner `Ok` means
//     "contract code ran and returned a typed error this client understands".
//   - A *missing authorization* is a host error raised before the contract's
//     own error path is reachable, so it comes back as
//     `Err(Err(InvokeError::Abort))` — not an `Error` variant at all, and
//     nothing that could be added to the `Error` enum would change that.
//
// So `assert!(res.is_err())` alone does not distinguish "the caller wasn't
// authorized" from "the contract rejected the argument", which for an auth
// test is the entire claim. The assertions below match the exact shape.
// ---------------------------------------------------------------------

/// What `try_add_asset` / `try_remove_asset` return. The nesting is the
/// client binding's, not this contract's: the outer `Result` is whether the
/// invocation succeeded, the inner `Ok` arm is the decoded return value, and
/// the inner `Err` arm distinguishes a typed contract `Error` from a host
/// `InvokeError`.
type WhitelistCallResult = Result<Result<(), ConversionError>, Result<Error, InvokeError>>;

/// Authorizes exactly one invocation by exactly one address, and nothing else.
///
/// Deliberately not `env.mock_all_auths()`: that makes every `require_auth()`
/// succeed regardless of who signed, which is precisely the condition these
/// tests exist to detect. Passing a non-admin as `signer` is what "a
/// non-admin calls this function" means for an entry point like `add_asset`
/// that takes no caller argument — the signed auth entry is the caller.
fn whitelist_call_as(
    env: &Env,
    vault: &LumensVaultClient,
    signer: &Address,
    fn_name: &'static str,
    asset: &Address,
) -> WhitelistCallResult {
    let invoke = MockAuthInvoke {
        contract: &vault.address,
        fn_name,
        args: (asset.clone(),).into_val(env),
        sub_invokes: &[],
    };
    let auths = [MockAuth {
        address: signer,
        invoke: &invoke,
    }];
    let scoped = vault.mock_auths(&auths);
    match fn_name {
        "add_asset" => scoped.try_add_asset(asset),
        "remove_asset" => scoped.try_remove_asset(asset),
        other => panic!("whitelist_call_as does not handle {other}"),
    }
}

/// Asserts a call was authorized and completed. Unwraps both layers of
/// `WhitelistCallResult` so callers don't have to, and so the inner
/// `#[must_use]` decode result isn't silently dropped.
fn assert_authorized(result: WhitelistCallResult, what: &str) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => panic!("{what} succeeded but its return value failed to decode: {e:?}"),
        Err(Ok(e)) => panic!("{what} was authorized but the contract rejected it: {e:?}"),
        Err(Err(e)) => panic!("{what} failed at the host level: {e:?} — check the mocked auth"),
    }
}

/// Asserts a call failed specifically because authorization was missing,
/// rather than for any other reason. See the block comment above for why
/// this is `Err(Err(Abort))` and not one of the contract's `Error` variants.
fn assert_unauthorized(result: WhitelistCallResult) {
    match result {
        Err(Err(InvokeError::Abort)) => {}
        Err(Ok(e)) => panic!(
            "expected an authorization failure, but the contract returned its own error: {e:?} \
             — that means the call was authorized and failed for a different reason"
        ),
        Err(Err(other)) => panic!("expected InvokeError::Abort, got {other:?}"),
        Ok(_) => panic!("expected an authorization failure, but the call succeeded"),
    }
}

/// Registers a vault and returns it alongside a real token contract address
/// to use as the whitelist subject. The constructor's `admin.require_auth()`
/// is satisfied under `mock_all_auths()`, which is then cleared so that every
/// call under test runs with only the auth it is explicitly given.
fn setup_whitelist_fixture(env: &Env, admin: &Address) -> (LumensVaultClient<'static>, Address) {
    env.mock_all_auths();
    let vault_client = setup(env, admin, 10);

    let token_admin = Address::generate(env);
    let (token_client, _) = create_token_contract(env, &token_admin);
    let asset = token_client.address.clone();

    // From here on, authorization is granted per call via `whitelist_call_as`.
    env.set_auths(&[]);

    (vault_client, asset)
}

#[test]
fn test_admin_can_add_and_remove_asset() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let (vault_client, asset) = setup_whitelist_fixture(&env, &admin);

    assert!(
        !vault_client.is_whitelisted(&asset),
        "an asset should not be whitelisted before the admin adds it"
    );

    assert_authorized(
        whitelist_call_as(&env, &vault_client, &admin, "add_asset", &asset),
        "admin's add_asset",
    );
    assert!(
        vault_client.is_whitelisted(&asset),
        "is_whitelisted should report true after the admin added the asset"
    );

    assert_authorized(
        whitelist_call_as(&env, &vault_client, &admin, "remove_asset", &asset),
        "admin's remove_asset",
    );
    assert!(
        !vault_client.is_whitelisted(&asset),
        "is_whitelisted should report false after the admin removed the asset"
    );
}

#[test]
fn test_add_asset_rejects_non_admin_and_leaves_asset_unlisted() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let (vault_client, asset) = setup_whitelist_fixture(&env, &admin);

    assert!(!vault_client.is_whitelisted(&asset));

    let res = whitelist_call_as(&env, &vault_client, &attacker, "add_asset", &asset);
    assert_unauthorized(res);

    // The point of the test: not merely that the call errored, but that no
    // part of it took effect. A rejected add must leave the asset unlisted,
    // so a later `deposit` still fails with `AssetNotWhitelisted`.
    assert!(
        !vault_client.is_whitelisted(&asset),
        "a rejected add_asset must not whitelist the asset"
    );
    assert_eq!(
        vault_client.get_admin_address(),
        admin,
        "a rejected add_asset must not disturb the admin either"
    );
}

#[test]
fn test_remove_asset_rejects_non_admin_and_leaves_asset_whitelisted() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let (vault_client, asset) = setup_whitelist_fixture(&env, &admin);

    assert_authorized(
        whitelist_call_as(&env, &vault_client, &admin, "add_asset", &asset),
        "admin's add_asset",
    );
    assert!(vault_client.is_whitelisted(&asset));

    let res = whitelist_call_as(&env, &vault_client, &attacker, "remove_asset", &asset);
    assert_unauthorized(res);

    // Again, state rather than the error is the claim: an unauthorized
    // delisting must not be able to block deposits of a legitimate asset.
    assert!(
        vault_client.is_whitelisted(&asset),
        "a rejected remove_asset must leave the asset whitelisted"
    );
    assert_eq!(
        vault_client.get_admin_address(),
        admin,
        "a rejected remove_asset must not disturb the admin either"
    );
}
