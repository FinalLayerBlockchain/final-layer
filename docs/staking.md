# Staking

The staking pool contract (`fl_staking_pool v5`) is deployed to each validator account. Delegators deposit FLC and the pool manages staked balances, lockups, and unbonding.

## Parameters

Lockup period: 48 hours (`LOCKUP_NS = 4 * 43200 * 1_000_000_000` nanoseconds).
Unbonding period: 4 epochs after unstaking (roughly another 48 hours).
Epoch length: 43,200 blocks.
Deposit fee: 10 bps (0.1%), deducted before staking.
Claim fee: 10 bps (0.1%), deducted from claimed rewards.

## Lifecycle

**deposit_and_stake()** — Deposits FLC and stakes it immediately. Deducts the deposit fee (0.1%) before computing shares. Sets `unlock_timestamp_ns` to 48 hours from now. If this is the first deposit into an empty pool, `last_locked_balance` is synced to the current protocol-locked amount before `internal_ping()` runs, preventing phantom rewards for the first delegator.

**compound()** — Reinvests accrued rewards back into staked principal without withdrawing them. Increases staked balance and resets the 48-hour lock. Equivalent to claim_rewards followed by re-deposit, but more gas-efficient and atomic.

**claim_rewards()** — Transfers accrued rewards (minus the claim fee of 0.1%) to the caller's account. Requires the 48-hour lock to have expired. Calling this resets the lock, so avoid it if you plan to unstake soon.

**unstake(amount)** — Moves a specific amount from staked to unstaked balance. Requires the 48-hour lock to have expired. Sets `unstake_available_epoch` to current epoch + 4.

**unstake_all()** — Moves your entire staked balance to unstaked balance. Requires the 48-hour lock to have expired. A sub-FLC dust residue (< 1 FLC) may remain from double floor-rounding in share math — this is harmless and stays in the pool as a tiny share-price lift.

**withdraw_all()** — Sends unstaked balance back to your wallet. Requires the 4-epoch unbonding period to have completed (`unstake_available_epoch <= current_epoch`).

## Enforcement

The lock check (applies to unstake, unstake_all, claim_rewards):
```rust
require!(
    env::block_timestamp() >= self.unlock_timestamp_ns,
    "Stake is still locked"
);
```

The unbonding check (applies to withdraw_all):
```rust
require!(
    account.unstaked_available_epoch_height <= env::epoch_height(),
    "The unstaked balance is not yet available due to unbonding period"
);
```

Both checks happen in the contract before any balance moves. Transactions that call too early are rejected on-chain.

## PQC key registration

Keys registered with the staking pool go through `parse_key_string()` which enforces exact byte lengths: 897 for FN-DSA, 1952 for ML-DSA, 32 for SLH-DSA. Wrong length panics with a descriptive message. Unknown algorithms also panic. No silent truncation.

## Version history

v1 introduced the contract but had wrong key bytes (missing Borsh length prefix). v2 fixed the encoding. v3 added the lockup mechanism. v4 added exact-length key validation. v5 fixed the first-delegator phantom reward bug (bootstrap fix).
