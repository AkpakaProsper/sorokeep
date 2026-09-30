# Issue #825 Implementation Demonstration

## Summary

Implemented explicit build dependency checking for the v2 fixture to prevent confusing errors when running `cargo test` from a clean checkout.

## What Was Changed

### 1. Created `build.rs` (lumens-vault/contracts/lumens-vault/build.rs)

A build script that checks for the fixture wasm at compile time and provides a clear, actionable error message if it's missing.

### 2. Created Test Script (lumens-vault/scripts/test-contract.ps1)

A PowerShell script that automates the correct build order:
- Builds the v2 fixture first
- Then runs cargo test

Works on Windows, macOS, and Linux with PowerShell 5.1+ or pwsh 7.x.

### 3. Updated README (lumens-vault/README.md)

Changed the "Build and test" section to:
- Recommend the test script as the primary entry point
- Keep manual instructions as an alternative
- Explain why the fixture must be built first

## Demonstration: Build Fails with Clear Message

### Before the Fixture is Built

Running `cargo build` from the main contract directory without the fixture:

```
error: failed to run custom build command for `lumens-vault v0.1.0`

Caused by:
  process didn't exit successfully: `build-script-build` (exit status: 101)
  --- stderr
  
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    ERROR: V2 fixture wasm not found
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  
  The upgrade test needs the v2 fixture compiled first.
  
  Run this command to build it:
  
    cd contracts/lumens-vault-v2-fixture && stellar contract build
  
  Or use the test script that handles it automatically:
  
    ./lumens-vault/scripts/test-contract.ps1
  
  See docs/testing.md for the full walkthrough.
  
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  
  thread 'main' panicked at build.rs:27:9:
  Build failed: fixture wasm missing
```

**Result:** Clear, actionable error message that:
- ✅ Names the exact problem (V2 fixture wasm not found)
- ✅ Provides the exact command to fix it
- ✅ Offers an alternative (the test script)
- ✅ Points to documentation for more context

### Comparison to Old Behavior

**OLD (tribal knowledge only):**
```
error: No such file or directory (os error 2)
   --> src/test.rs:192:5
    |
192 | /     soroban_sdk::contractimport!(
193 | |         file = "../lumens-vault-v2-fixture/target/wasm32v1-none/release/lumens_vault_v2_fixture.wasm"
194 | |     );
    | |_____^
```

**NEW (explicit and helpful):**
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ERROR: V2 fixture wasm not found
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

The upgrade test needs the v2 fixture compiled first.

Run this command to build it:

  cd contracts/lumens-vault-v2-fixture && stellar contract build

Or use the test script that handles it automatically:

  ./lumens-vault/scripts/test-contract.ps1

See docs/testing.md for the full walkthrough.
```

## Approach Justification

**Chose:** Build script (`build.rs`) with clear error message + convenience test script

**Why:**

1. **Compile-time detection**: Catches the problem before any test runs, at the exact point where the dependency is needed
2. **Zero workflow changes**: `cargo test` still works naturally - no need to remember a different command
3. **Standard Rust pattern**: Build scripts are a well-known mechanism for build-time dependencies
4. **Clear error messaging**: Can provide detailed, formatted instructions
5. **Convenience option**: Test script for those who prefer automation
6. **No vendoring**: Keeps the repo clean (wasm binaries stay gitignored)

**Alternative considered:** Test-only script that builds fixture then runs tests
- **Rejected because**: Would require changing everyone's workflow from `cargo test` to `./test-script.ps1`
- **Compromise**: Provide both - build.rs catches errors, script offers convenience

## Acceptance Criteria Met

✅ **Pick one mechanism and justify it**: Chose build.rs, justified above

✅ **Running from clean checkout produces working run or clear error**: 
   - Without fixture: Clear error with exact fix command
   - With test script: Builds fixture automatically then runs tests

✅ **README uses this entry point**: Updated to recommend test script, show manual steps

✅ **Demonstrate it**: 
   - Deleted fixture target/
   - Ran build
   - Got clear error message (shown above)

## Files Changed

1. **lumens-vault/contracts/lumens-vault/build.rs** (new)
2. **lumens-vault/scripts/test-contract.ps1** (new)
3. **lumens-vault/README.md** (edited)

## How to Test This Implementation

```powershell
# 1. Clean state - remove fixture if it exists
rm -rf lumens-vault/contracts/lumens-vault-v2-fixture/target

# 2. Try to build - should get clear error
cd lumens-vault/contracts/lumens-vault
cargo build

# 3. Follow the error message instructions
cd ../lumens-vault-v2-fixture
stellar contract build

# 4. Now cargo test should work
cd ../lumens-vault
cargo test

# Or use the convenience script from lumens-vault root:
./scripts/test-contract.ps1
```

## Non-Goals Respected

❌ Did not vendor the fixture wasm into the repo
✅ Fixture target/ remains in .gitignore
