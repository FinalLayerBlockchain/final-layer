# Final Layer Staking Pool — Bug History and Fixes

**Contract:** `fl_staking_pool` (deployed to each validator's account)  
**Current version:** v5  
**Date of full resolution:** 2026-04-06

---

## Overview

The Final Layer staking pool went through five iterations to reach a fully working state. Three separate bugs were discovered and fixed across those versions. This document explains what each bug was, why it happened, and what the fix was.

---

## Bug 1 — Wrong Key Type Bytes (v1 → v3)

### What broke
Every call to `deposit_and_stake()` silently succeeded at the contract level but failed when the contract tried to call `promise_batch_action_stake()`. The validator's staked balance never changed on-chain.

### Root cause
The `parse_key_string()` function serializes a PQC public key into the Borsh format that `near_crypto::PublicKey` expects. The first byte of that encoding is the `KeyType` discriminant:

```
near_crypto::KeyType:
  ED25519   = 0
  SECP256K1 = 1
  MLDSA     = 2
  FNDSA     = 3
  SLHDSA    = 4
```

The v1 contract had the wrong values:

```rust
// v1 — WRONG (collides with ED25519 and SECP256K1)
"mldsa"  => 0,
"fndsa"  => 1,
"slhdsa" => 2,
```

When the runtime received an ML-DSA key encoded with type byte `0`, it read it as an ED25519 key. ED25519 keys are 32 bytes; an ML-DSA key is 1952 bytes. The Borsh decoder immediately rejected the malformed payload. The stake action was reverted.

### Fix (v3)
Corrected the key type bytes to match `near_crypto::KeyType`:

```rust
// v3 — CORRECT
"mldsa"  => 2,
"fndsa"  => 3,
"slhdsa" => 4,
```

---

## Bug 2 — Missing Borsh Vec\<u8\> Length Prefix (v3 → v4)

### What broke
After fixing the key type bytes in v3, `deposit_and_stake()` still failed with a Borsh deserialization error in `promise_batch_action_stake()`.

### Root cause
In `near_crypto`, ED25519 and SECP256K1 public keys are stored as fixed-size byte arrays. Their Borsh encoding is simply:

```
[key_type_byte(1)] + [raw_bytes]
```

But ML-DSA, FN-DSA, and SLH-DSA public keys are stored as `Vec<u8>`. Borsh encodes `Vec<u8>` with a 4-byte little-endian length prefix:

```
[key_type_byte(1)] + [length_u32_LE(4)] + [raw_bytes]
```

The v3 contract was still using the fixed-array encoding (no length prefix) for PQC keys. The runtime's Borsh decoder read 4 bytes of the key as the "rest of the header" and shifted all subsequent field offsets, causing deserialization to fail.

### Fix (v4)
Added the 4-byte Borsh length prefix for all PQC algorithms:

```rust
// v4 — CORRECT encoding for PQC public keys
match algo {
    "mldsa" | "fndsa" | "slhdsa" => {
        let len_bytes = (key_bytes.len() as u32).to_le_bytes();
        result.extend_from_slice(&len_bytes);  // 4-byte LE length prefix
    }
    _ => {}
}
result.extend_from_slice(&key_bytes);
```

Correct final encoding for FN-DSA (897-byte public key):
```
[0x03]  [0x81, 0x03, 0x00, 0x00]  [897 bytes of key data]
  ^key   ^---- 897 in LE u32 ----^
  type
```
Total: 902 bytes.

---

## Bug 3 — First-Delegator Phantom Reward (v4 → v5)

### What broke
The very first person to stake into a freshly-deployed pool would see an astronomically large `rewards_earned` value — equal to the validator's entire locked stake (hundreds of millions of FLC).

### Root cause
The contract uses share-based accounting. When a delegator deposits, an internal function `internal_ping()` runs first to credit any new validator rewards to existing share holders. It does this by checking:

```rust
let new_reward = current_locked_balance - self.last_locked_balance;
// credit new_reward to all shareholders proportionally
```

`last_locked_balance` is supposed to track the locked balance as of the last known state. But on a brand-new pool, `last_locked_balance` starts at **zero**. The validator already has a large locked balance from its own staking (e.g. 300 million FLC). So the first time `internal_ping()` runs:

```
new_reward = 300_000_000 FLC - 0 = 300_000_000 FLC
```

This entire amount is credited as "rewards" to the first delegator, even though none of it was earned after the pool was deployed.

### Fix (v5)
On the very first deposit (when `total_stake_shares == 0`), sync `last_locked_balance` to the current locked balance before running `internal_ping()`:

```rust
// v5 bootstrap fix (lib.rs line 146)
if self.total_stake_shares == 0 {
    self.last_locked_balance = env::account_locked_balance().as_yoctonear();
}
```

This sets the baseline to "what the validator already had locked before the pool opened." Only locked-balance increases *after* this point count as delegator rewards.

**Test confirmation:** T0-Bootstrap PASSED in the live test suite — first depositor saw ~999 FLC (1000 deposit minus 0.1% fee), not hundreds of millions.

---

## Bug 4 — Truncation Instead of Rejection (v4, discovered via ChatGPT review → v5)

### What existed
The v4 contract silently accepted keys that were larger than the expected size by truncating them:

```rust
// v4 — TRUNCATION (silent, dangerous)
if key_bytes.len() > pk_len { key_bytes.truncate(pk_len); }
```

### Why it's a problem
A caller passing an oversized key (e.g. accidentally submitting the combined pk||sk blob instead of just the public key) would get their transaction accepted and a silently wrong key registered. The staking action would then fail later at the protocol level with a cryptographic verification error, leaving the pool in an ambiguous state.

ChatGPT flagged this during a code review, noting that the live source showed truncation while the response claimed strict validation. The challenge was valid — source beats prose.

### Fix (v5)
Replace truncation with a hard panic that rejects wrong-size keys immediately:

```rust
// v5 — REJECT on wrong size
if key_bytes.len() != pk_len {
    panic!("Key must be exactly {} bytes for {}, got {}", pk_len, algo, key_bytes.len());
}
```

This makes the failure fast, explicit, and caught before any state changes.

---

## Summary Table

| Version | What changed | Result |
|---------|-------------|--------|
| v1 | Initial deployment | Staking broken (wrong key type bytes) |
| v2 | Internal restake no-op workaround | Deposit works, no on-chain restake |
| v3 | Key bytes corrected (mldsa=2, fndsa=3, slhdsa=4) | Still broken (missing Borsh prefix) |
| v4 | Borsh Vec\<u8\> length prefix added | Full staking works; bootstrap + truncation bugs remain |
| v5 | Bootstrap fix + exact-length validation | All bugs resolved; 13/13 test suite passed |

---

## Live Test Results (v5)

Full test suite run on isolated testnet, 2026-04-06:

| Test | Result | What it proves |
|------|--------|---------------|
| T0-Bootstrap | PASS | First depositor gets ~999 FLC, not validator's locked balance |
| T1-DepositFee | PASS | 0.1% fee correctly deducted |
| T2-LockupReject | PASS | Unstake rejected within 48h lock window |
| T3-RewardAccrual | PASS | Rewards accumulate over epochs |
| T4-Compound | PASS | compound() reinvests rewards, resets lockup |
| T5-AddStake | PASS | Second deposit increases balance correctly |
| T6-ClaimRewards | PASS | claim_rewards() works after lockup expires |
| T7-PartialUnstake | PASS | unstake(amount) moves exact amount to unbonding |
| T8-PrematureWithdraw | PASS | withdraw rejected before 4-epoch unbonding |
| T9-FullUnstake | PASS | unstake_all() leaves only sub-FLC dust (expected) |
| T10-WithdrawAfterUnbond | PASS | withdraw_all() succeeds after 4 epochs |
| T11-DustCheck | PASS | Residue < 1 FLC (normal share-math rounding) |
| T12-APY | PASS | Rewards are non-negative; pool operational |

See `staking_pool_test_report.txt` for full output.
