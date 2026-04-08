/*!
 * Final Layer Staking Pool Contract v7 — Multi-AI Re-Audit Response (v6 audit)
 * (near-sdk 5.x, legacy collections)
 *
 * v7 changes (from v6) — Kimi, Perplexity, Gemini, ChatGPT re-audit of v6:
 *
 *   FIXED — [Perplexity] migrate() no-op path incorrectly clears pending owner
 *     Scenario: pool has an active two-step ownership transfer in flight (owner
 *     called propose_ownership(), new owner has not yet accepted).  Operator
 *     deploys v7 over v5/v6 (same layout, no migration needed) but calls
 *     migrate() out of habit.  The current-layout path was:
 *
 *       if let Some(current) = env::state_read::<StakingPool>() {
 *           env::storage_write(MIGRATION_VERSION_KEY, &[7]);
 *           write_pending_owner(None);   // ← silently killed the transfer
 *           return current;
 *       }
 *
 *     write_pending_owner(None) was copied from the v11 path (where clearing
 *     stale pre-migration ownership is genuinely needed) but has no business
 *     being in the no-op path — there is nothing stale to clear.
 *     Fix: remove write_pending_owner(None) from the current-layout path.
 *     Keep it only in the true v11 → v7 migration path where it belongs.
 *
 * ─────────────────────────────────────────────────────────────────────────────
 * REJECTED fixes from v6 re-audit (with reasoning):
 *
 *   NOT FIXED — [ChatGPT / Perplexity] Floor-burn rounding
 *     ChatGPT now agrees: "not a practical exploit today."  burn=0 requires
 *     amt < share_price.  With MIN_STAKE = 10^24, share price starts at 1
 *     yoctonear and grows ~10-15% per year.  Gas cost per tx ≈ 10^23 yoctonear.
 *     Maximum extraction per tx ≈ 1-2 yoctonear.  Gas exceeds extraction by
 *     10^21×.  Economically infeasible.  Rejected 7 times.
 *
 *   NOT FIXED — [ChatGPT] internal_restake() max(total_staked, last_locked)
 *     Intentional design.  Rejected 6 times.
 *
 *   NOT FIXED — [ChatGPT] Detached fee transfer ambiguity
 *     Acceptable risk.  Failed transfers stay in contract liquid balance and
 *     are attributed as pool rewards on the next internal_ping().  Rejected 7×.
 *
 *   NOT FIXED — [ChatGPT] lock_upgrades() "success" semantics
 *     ChatGPT themselves say "probably acceptable."  #[private] + is_promise_
 *     success() on a direct callback is the standard NEAR pattern.  The deployer
 *     should verify is_upgrades_locked() after calling.  Documentation-only.
 *
 *   NOT FIXED — [Kimi C1] migrate() paused=true "undefined behavior"
 *     Factually incorrect.  Traced byte layout: on v11 state with paused=true
 *     (0x01), env::state_read::<StakingPool>() reads 8 base fields (981 bytes),
 *     reads Option discriminant 0x01 (Some), then tries to read PendingFeeUpdate
 *     (12 bytes) from 0 remaining bytes → EOF → state_read returns None.
 *     Both paused=false and paused=true correctly fall through to OldStakingPool.
 *     Kimi's proposed require!(!old.paused) would be an anti-fix: paused pools
 *     should be migrated (pause functionality was the security risk itself).
 *
 *   NOT FIXED — [Kimi C2] on_lock_upgrades_complete promise manipulation
 *     Kimi self-diagnoses: "this doesn't add real security" and "probably
 *     overkill."  The #[private] + is_promise_success() pattern is correct.
 *
 *   NOT FIXED — [Kimi H1] Owner fee_shares = 0 edge case
 *     Kimi traces realistic numbers: "The math checks out. The v5 fix is
 *     correct."  No exploit found.
 *
 *   NOT FIXED — [Kimi H2] Over-stake on slash
 *     Kimi: "This is intentional per the comments. No change needed."
 *
 *   NOT FIXED — [Kimi H3] Lockup bypass via multiple accounts
 *     Kimi: "This is not an exploit — it's how staking pools work."
 *
 *   NOT FIXED — [Kimi M1] Optional slippage / MEV
 *     4-epoch lockup prevents MEV from being profitable.  Rejected 4×.
 *
 *   NOT FIXED — [Kimi M2] Ownership proposal no expiry
 *     cancel_ownership_transfer() already provides an escape hatch.  Owner
 *     can cancel and re-propose at any time.  No stuck state possible.
 *
 *   NOT FIXED — [Kimi M3/L1-L3/I1-I3] Events, view rounding, storage cleanup
 *     All informational or out of scope.
 *
 *   NOT FIXED — [Perplexity] internal_ping() zero-locked skip
 *     Intentional bootstrap behavior.  Panicking would brick the contract if
 *     the validator ever fully unstakes.  Rejected every round.
 *
 *   NOT FIXED — [Perplexity] unwrap_or_default → expect in on_withdraw_complete
 *     Changing to expect() would make the failure mode WORSE: if a delegator
 *     record is unexpectedly missing, expect panics, state is NOT restored,
 *     user loses their funds.  unwrap_or_default is the correct safe fallback.
 *
 *   NOT FIXED — [Gemini] All findings
 *     Gemini described features that do not exist in v6: "Reward Sandwich
 *     Mitigation," "NIP-299 standard events," "StakeEvent/UnstakeEvent/
 *     RewardClaimEvent."  Gemini analyzed a hallucinated version of the contract.
 *     None of Gemini's v6 findings apply to the actual code.
 *
 * ─────────────────────────────────────────────────────────────────────────────
 * Full prior change history:
 *
 * v6 changes (from v5):
 *   FIXED — migrate() try-current-layout first (graceful v4/v5 no-op path)
 *   FIXED — is_restake_healthy() docstring: length-only check disclaimer
 *
 * v5 changes (from v4):
 *   FIXED — Migration double-call guard (MIGRATION_VERSION_KEY)
 *   FIXED — is_restake_healthy() view method
 *   FIXED — Owner fee principal accounting drift (REAL BUG in v4)
 *   FIXED — ph == 0 guard in shares_for_amount_post_reduce()
 *   FIXED — Wildcard arm in parse_key_string length match
 *   FIXED — lock_upgrades() → Promise + callback
 *
 * v4 changes (from v3):
 *   FIXED — CALLBACK_GAS 5→10 TGas
 *   FIXED — deposit_and_stake(): zero-share deposit guard
 *   FIXED — shares_for_amount_post_reduce(): net_reward denominator
 *   FIXED — read_pending_owner(): safe UTF-8/.ok() decode
 *   FIXED — parse_key_string() wildcard: env::panic_str()
 *   FIXED — propose_ownership(): self-transfer guard
 *   FIXED — migrate(): write_pending_owner(None)
 *   FIXED — withdraw_all(): serde_json::json! for callback args
 *
 * v3 changes (from v2):
 *   FIXED — muldiv128 for principal calculation (overflow)
 *   FIXED — Two-step ownership (propose/accept/cancel)
 *   FIXED — withdraw_all() callback pattern (M5)
 *
 * v2 changes (from v11):
 *   - Removed pause/unpause, fix_pool_accounting, fix_delegator_*
 *   - Hard cap claim fee at 10%, fee timelock 48h
 *   - Added lock_upgrades(), staking key length validation
 *   - deposit_and_stake() slippage guard, muldiv128 for fees
 */

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near, require, AccountId, Gas, NearToken, Promise, PublicKey};

// ── Constants ─────────────────────────────────────────────────────────────────

const LOCKUP_NS: u64             = 4 * 43_200 * 1_000_000_000;
const NUM_EPOCHS_TO_UNLOCK: u64  = 4;
const MAX_DEPOSIT_FEE_BPS: u16   = 10;    // 0.1% hard cap
const MAX_CLAIM_FEE_BPS: u16     = 1000;  // 10% hard cap
const MIN_STAKE: u128            = 1_000_000_000_000_000_000_000_000; // 1 FLC
const FEE_TIMELOCK_NS: u64       = 48 * 3_600 * 1_000_000_000; // 48h

// Gas budget for callbacks.  on_withdraw_complete: ~3-5 TGas for one
// LookupMap read + write.  on_lock_upgrades_complete: one state write.
// 10 TGas provides 2× safety margin for both.
const CALLBACK_GAS: Gas = Gas::from_tgas(10);

// Expected Borsh-encoded byte lengths for each supported PQC key type.
const FNDSA_KEY_BORSH_LEN: usize  = 1 + 4 + 897;
const MLDSA_KEY_BORSH_LEN: usize  = 1 + 4 + 1952;
const SLHDSA_KEY_BORSH_LEN: usize = 1 + 4 + 32;

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Default)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct Delegator {
    pub stake_shares:            u128,
    pub principal:               u128,
    pub unstaked_balance:        u128,
    pub unstake_available_epoch: u64,
    pub unlock_timestamp_ns:     u64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AccountView {
    pub staked_balance:          U128,
    pub unstaked_balance:        U128,
    pub total_balance:           U128,
    pub principal:               U128,
    pub rewards_earned:          U128,
    pub can_withdraw:            bool,
    pub is_locked:               bool,
    pub unlock_timestamp_ns:     u64,
    pub unstake_available_epoch: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct PoolFees {
    pub deposit_fee_bps: u16,
    pub claim_fee_bps:   u16,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct PendingFeeUpdate {
    pub deposit_fee_bps:   u16,
    pub claim_fee_bps:     u16,
    pub execution_time_ns: u64,
}

// ── Raw WASM host calls for PQC staking key support ───────────────────────────

#[cfg(target_arch = "wasm32")]
unsafe fn sys_promise_batch_create(account_id: &str) -> u64 {
    extern "C" {
        fn promise_batch_create(account_id_len: u64, account_id_ptr: u64) -> u64;
    }
    promise_batch_create(account_id.len() as u64, account_id.as_ptr() as u64)
}

#[cfg(target_arch = "wasm32")]
unsafe fn sys_promise_batch_action_stake(promise_idx: u64, amount: u128, pk_bytes: &[u8]) {
    extern "C" {
        fn promise_batch_action_stake(
            promise_index:  u64,
            amount_ptr:     u64,
            public_key_len: u64,
            public_key_ptr: u64,
        );
    }
    let le = amount.to_le_bytes();
    promise_batch_action_stake(
        promise_idx,
        le.as_ptr() as u64,
        pk_bytes.len() as u64,
        pk_bytes.as_ptr() as u64,
    );
}

// ── Migration ─────────────────────────────────────────────────────────────────
//
// On-chain state written by v11 has 9 Borsh fields:
//   owner_id, staking_key_bytes, deposit_fee_bps, claim_fee_bps,
//   total_staked_balance, total_stake_shares, last_locked_balance,
//   delegators, paused
//
// v7 (via v2/v3/v4/v5/v6) uses 10 fields (same layout as v3–v6).
// Deploying v7 over v3–v6 state does NOT require migrate().
// migrate() is only needed for v11 (9 fields) → v7 upgrades.

#[derive(BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
struct OldStakingPool {
    owner_id:             AccountId,
    staking_key_bytes:    Vec<u8>,
    deposit_fee_bps:      u16,
    claim_fee_bps:        u16,
    total_staked_balance: u128,
    total_stake_shares:   u128,
    last_locked_balance:  u128,
    delegators:           LookupMap<AccountId, Delegator>,
    /// Present in v11 and earlier; removed in security-hardened v2.
    /// Must be included so Borsh reads all 982 state bytes correctly.
    paused:               bool,
}

// ── Contract ──────────────────────────────────────────────────────────────────

// pending_owner lives in storage slot "po" rather than in the main struct,
// preserving Borsh layout compatibility across all versions.
const PENDING_OWNER_KEY:     &[u8] = b"po";

// Migration version marker.  Written once on the first successful migrate().
const MIGRATION_VERSION_KEY: &[u8] = b"mv";

// v4 fix: safe decode — returns None on malformed bytes rather than panicking.
fn read_pending_owner() -> Option<AccountId> {
    env::storage_read(PENDING_OWNER_KEY).and_then(|bytes| {
        String::from_utf8(bytes)
            .ok()
            .and_then(|s| AccountId::try_from(s).ok())
    })
}

fn write_pending_owner(owner: Option<&AccountId>) {
    match owner {
        Some(o) => { env::storage_write(PENDING_OWNER_KEY, o.as_bytes()); }
        None    => { env::storage_remove(PENDING_OWNER_KEY); }
    }
}

#[near(contract_state)]
pub struct StakingPool {
    pub owner_id:             AccountId,
    pub staking_key_bytes:    Vec<u8>,
    pub deposit_fee_bps:      u16,
    pub claim_fee_bps:        u16,
    pub total_staked_balance: u128,
    pub total_stake_shares:   u128,
    pub last_locked_balance:  u128,
    pub delegators:           LookupMap<AccountId, Delegator>,
    pub pending_fee_update:   Option<PendingFeeUpdate>,
    pub upgrades_locked:      bool,
    // NOTE: pending_owner lives in storage slot "po" (not a struct field).
    // NOTE: migration marker lives in storage slot "mv".
}

impl Default for StakingPool {
    fn default() -> Self {
        panic!("StakingPool must be initialized via new()")
    }
}

#[near]
impl StakingPool {

    /// Migration supporting two upgrade paths:
    ///   Path A — current layout (v3/v4/v5/v6 → v7, no data change needed):
    ///     env::state_read::<StakingPool>() succeeds, writes marker, returns as-is.
    ///     v7 fix: does NOT clear pending_owner — an in-flight ownership transfer
    ///     must be preserved.  Only the v11 path clears it (where stale pre-migration
    ///     state must be discarded).
    ///   Path B — v11 (9-field state) → v7:
    ///     env::state_read::<StakingPool>() returns None (EOF on 10th field read).
    ///     Falls through to OldStakingPool, clears stale pending owner, migrates.
    ///
    /// Why paused=true v11 state is safe: byte 982 of v11 state (paused) is read
    /// as the Option<PendingFeeUpdate> discriminant.  Whether 0x00 (None) or 0x01
    /// (Some), in both cases deserialization tries to read more bytes than exist
    /// → EOF → state_read returns None → falls to OldStakingPool path correctly.
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        require!(
            env::storage_read(MIGRATION_VERSION_KEY).is_none(),
            "Already migrated — migrate() can only be called once"
        );

        // Path A: already at v3–v6 layout. Write marker and return unchanged.
        // v7 fix: do NOT call write_pending_owner(None) here — any active
        // ownership transfer must be preserved across a no-op migration.
        if let Some(current) = env::state_read::<StakingPool>() {
            env::storage_write(MIGRATION_VERSION_KEY, &[7]);
            return current;
        }

        // Path B: v11 → v7.  Clear stale pending owner from pre-migration state.
        write_pending_owner(None);

        let old = env::state_read::<OldStakingPool>()
            .expect("State not found — contract uninitialized or unknown layout");

        env::storage_write(MIGRATION_VERSION_KEY, &[7]);

        Self {
            owner_id:             old.owner_id,
            staking_key_bytes:    old.staking_key_bytes,
            deposit_fee_bps:      old.deposit_fee_bps,
            claim_fee_bps:        old.claim_fee_bps,
            total_staked_balance: old.total_staked_balance,
            total_stake_shares:   old.total_stake_shares,
            last_locked_balance:  old.last_locked_balance,
            delegators:           old.delegators,
            pending_fee_update:   None,
            upgrades_locked:      false,
        }
    }

    #[init]
    pub fn new(
        owner_id: AccountId,
        staking_key: String,
        deposit_fee_bps: u16,
        claim_fee_bps: u16,
    ) -> Self {
        require!(deposit_fee_bps <= MAX_DEPOSIT_FEE_BPS,
            "deposit_fee_bps exceeds 0.1% maximum");
        require!(claim_fee_bps <= MAX_CLAIM_FEE_BPS,
            "claim_fee_bps exceeds 10% maximum");
        Self {
            owner_id,
            staking_key_bytes: parse_key_string(&staking_key),
            deposit_fee_bps,
            claim_fee_bps,
            total_staked_balance: 0,
            total_stake_shares: 0,
            last_locked_balance: 0,
            delegators: LookupMap::new(b"d".to_vec()),
            pending_fee_update: None,
            upgrades_locked: false,
        }
    }

    // ── User actions ────────────────────────────────────────────────────────

    #[payable]
    pub fn deposit_and_stake(&mut self, min_shares_out: Option<U128>) {
        if self.total_stake_shares == 0 {
            self.last_locked_balance = env::account_locked_balance().as_yoctonear();
        }
        self.internal_ping();

        let amount = env::attached_deposit().as_yoctonear();
        require!(amount >= MIN_STAKE, "Deposit must be >= 1 FLC");

        let fee = muldiv128(amount, self.deposit_fee_bps as u128, 10_000);
        let net = amount - fee;
        if fee > 0 {
            near_sdk::Promise::new(self.owner_id.clone())
                .transfer(NearToken::from_yoctonear(fee))
                .detach();
        }

        let shares = self.shares_for(net);

        // v4 fix: guard against deposits so small that shares round to zero.
        // Without this, the user's tokens are absorbed into the pool with no
        // shares minted and no way to recover them.
        require!(shares > 0, "Deposit too small to mint shares");

        if let Some(min_out) = min_shares_out {
            if min_out.0 > 0 {
                require!(shares >= min_out.0,
                    "Slippage: received fewer shares than minimum specified");
            }
        }

        let account_id = env::predecessor_account_id();
        let mut d = self.delegators.get(&account_id).unwrap_or_default();
        d.stake_shares += shares;
        self.total_staked_balance += net;
        self.total_stake_shares += shares;

        // M2 fix: compute principal from post-deposit share ratio via muldiv128
        // to ensure principal never exceeds staked value.
        // Note: d.principal += net would reintroduce the v11 rounding-gap bug:
        // when share price > 1, floor rounding means amount_for_shares(shares) < net,
        // so principal > staked → rewards = staked - principal < 0 (negative).
        let actual = if self.total_stake_shares > 0 {
            muldiv128(shares, self.total_staked_balance, self.total_stake_shares)
        } else {
            net
        };
        d.principal += actual;
        d.unlock_timestamp_ns = env::block_timestamp() + LOCKUP_NS;
        self.delegators.insert(&account_id, &d);

        self.internal_restake();
    }

    pub fn unstake(&mut self, amount: U128) {
        self.internal_ping();
        let account_id = env::predecessor_account_id();
        let mut d = self.delegators.get(&account_id).expect("No stake found");

        require!(
            env::block_timestamp() >= d.unlock_timestamp_ns,
            "Stake is still locked"
        );

        let amt = amount.0;
        let staked = self.amount_for_shares(d.stake_shares);
        require!(amt > 0 && amt <= staked, "Invalid unstake amount");

        let burn = self.shares_for_amount(amt);
        d.stake_shares = d.stake_shares.saturating_sub(burn);
        d.principal    = d.principal.saturating_sub(amt);
        d.unstaked_balance += amt;
        d.unstake_available_epoch = env::epoch_height() + NUM_EPOCHS_TO_UNLOCK;
        d.unlock_timestamp_ns = env::block_timestamp() + LOCKUP_NS;
        self.delegators.insert(&account_id, &d);

        self.total_staked_balance = self.total_staked_balance.saturating_sub(amt);
        self.total_stake_shares   = self.total_stake_shares.saturating_sub(burn);
        self.internal_restake();
    }

    pub fn unstake_all(&mut self) {
        let account_id = env::predecessor_account_id();
        let d = self.delegators.get(&account_id).expect("No stake found");
        let staked = self.amount_for_shares(d.stake_shares);
        require!(staked > 0, "Nothing to unstake");
        self.unstake(U128(staked));
    }

    /// M5 fix: withdraw_all() returns a Promise and chains a callback.
    /// If the transfer fails, on_withdraw_complete() restores the delegator's
    /// unstaked_balance and unstake_available_epoch exactly.
    /// v4 fix: callback args use serde_json::json!() for safe serialization.
    pub fn withdraw_all(&mut self) -> Promise {
        let account_id = env::predecessor_account_id();
        let mut d = self.delegators.get(&account_id).expect("No withdrawal found");
        require!(d.unstaked_balance > 0, "No unstaked balance");
        require!(
            env::epoch_height() >= d.unstake_available_epoch,
            "Still in unbonding period"
        );
        let amount = d.unstaked_balance;
        let saved_epoch = d.unstake_available_epoch;

        d.unstaked_balance = 0;
        d.unstake_available_epoch = 0;
        self.delegators.insert(&account_id, &d);

        let args = near_sdk::serde_json::json!({
            "account_id": account_id,
            "amount":     U128(amount),
            "saved_epoch": saved_epoch,
        })
        .to_string()
        .into_bytes();

        Promise::new(account_id)
            .transfer(NearToken::from_yoctonear(amount))
            .then(
                Promise::new(env::current_account_id())
                    .function_call(
                        "on_withdraw_complete".to_string(),
                        args,
                        NearToken::from_yoctonear(0),
                        CALLBACK_GAS,
                    )
            )
    }

    /// Callback for withdraw_all(). Restores user state if the transfer failed.
    /// Rollback is exact: only the two fields zeroed by withdraw_all() are
    /// restored.  No other fields are modified by withdraw_all(), and no code
    /// path in this contract deletes delegator records.
    /// #[private] ensures only the contract itself can invoke this.
    #[private]
    pub fn on_withdraw_complete(
        &mut self,
        account_id: AccountId,
        amount: U128,
        saved_epoch: u64,
    ) {
        if !near_sdk::is_promise_success() {
            let mut d = self.delegators.get(&account_id).unwrap_or_default();
            d.unstaked_balance += amount.0;
            d.unstake_available_epoch = saved_epoch;
            self.delegators.insert(&account_id, &d);
        }
    }

    pub fn claim_rewards(&mut self) {
        self.internal_ping();
        let account_id = env::predecessor_account_id();
        let mut d = self.delegators.get(&account_id).expect("No stake found");

        let staked  = self.amount_for_shares(d.stake_shares);
        let rewards = staked.saturating_sub(d.principal);
        require!(rewards > 0, "No rewards yet");

        let fee        = muldiv128(rewards, self.claim_fee_bps as u128, 10_000);
        let net_reward = rewards - fee;
        let burn       = self.shares_for_amount(rewards);

        d.stake_shares = d.stake_shares.saturating_sub(burn);
        d.unstaked_balance += net_reward;
        d.unstake_available_epoch = env::epoch_height() + NUM_EPOCHS_TO_UNLOCK;
        d.unlock_timestamp_ns = env::block_timestamp() + LOCKUP_NS;
        self.delegators.insert(&account_id, &d);

        if fee > 0 {
            let fee_shares = self.shares_for_amount_post_reduce(fee, net_reward, burn);

            // v5 fix [H2]: update pool totals BEFORE computing owner's principal.
            // Using amount_for_shares(fee_shares) after the update ensures
            // od.principal <= actual share value, preventing negative rewards.
            self.total_stake_shares = self.total_stake_shares
                .saturating_sub(burn)
                .saturating_add(fee_shares);
            self.total_staked_balance = self.total_staked_balance.saturating_sub(net_reward);

            let owner_id = self.owner_id.clone();
            let mut od = self.delegators.get(&owner_id).unwrap_or_default();
            od.stake_shares += fee_shares;
            od.principal    += self.amount_for_shares(fee_shares);
            self.delegators.insert(&owner_id, &od);
        } else {
            self.total_stake_shares   = self.total_stake_shares.saturating_sub(burn);
            self.total_staked_balance = self.total_staked_balance.saturating_sub(rewards);
        }
        self.internal_restake();
    }

    pub fn compound(&mut self) {
        self.internal_ping();
        let account_id = env::predecessor_account_id();
        let mut d = self.delegators.get(&account_id).expect("No stake found");

        let staked  = self.amount_for_shares(d.stake_shares);
        let rewards = staked.saturating_sub(d.principal);
        require!(rewards > 0, "No rewards to compound");

        // compound() resets the reward counter by updating principal to the
        // current staked value.  It does not mint new shares or move tokens —
        // rewards already accrue via share price growth.
        d.principal = staked;
        d.unlock_timestamp_ns = env::block_timestamp() + LOCKUP_NS;
        self.delegators.insert(&account_id, &d);
    }

    pub fn sync_principal(&mut self) {
        // Lowers principal to match staked value when staked < principal.
        // Corrects display after a slashing event without affecting actual
        // economic position.
        let account_id = env::predecessor_account_id();
        let mut d = self.delegators.get(&account_id).expect("No stake found");
        let staked = self.amount_for_shares(d.stake_shares);
        if staked < d.principal {
            d.principal = staked;
            self.delegators.insert(&account_id, &d);
        }
    }

    pub fn ping(&mut self) {
        self.internal_ping();
    }

    // ── Owner actions ───────────────────────────────────────────────────────

    pub fn update_staking_key(&mut self, new_staking_key: String) {
        require!(env::predecessor_account_id() == self.owner_id, "Owner only");
        self.staking_key_bytes = parse_key_string(&new_staking_key);
        self.internal_restake();
    }

    /// Propose a fee change.
    /// Fee DECREASES take effect immediately (always user-favorable).
    /// Fee INCREASES enter a 48-hour timelock before taking effect.
    pub fn propose_fee_update(&mut self, deposit_fee_bps: u16, claim_fee_bps: u16) {
        require!(env::predecessor_account_id() == self.owner_id, "Owner only");
        require!(deposit_fee_bps <= MAX_DEPOSIT_FEE_BPS,
            "deposit_fee_bps exceeds 0.1% maximum");
        require!(claim_fee_bps <= MAX_CLAIM_FEE_BPS,
            "claim_fee_bps exceeds 10% maximum");

        let is_increase = deposit_fee_bps > self.deposit_fee_bps
                       || claim_fee_bps   > self.claim_fee_bps;

        if is_increase {
            self.pending_fee_update = Some(PendingFeeUpdate {
                deposit_fee_bps,
                claim_fee_bps,
                execution_time_ns: env::block_timestamp() + FEE_TIMELOCK_NS,
            });
        } else {
            self.deposit_fee_bps = deposit_fee_bps;
            self.claim_fee_bps   = claim_fee_bps;
            self.pending_fee_update = None;
        }
    }

    /// Execute a pending fee increase after the 48h timelock has passed.
    /// Open to anyone — prevents owner from indefinitely blocking an update.
    pub fn execute_fee_update(&mut self) {
        let pending = self.pending_fee_update.as_ref()
            .expect("No pending fee update");
        require!(
            env::block_timestamp() >= pending.execution_time_ns,
            "Timelock not expired"
        );
        let update = self.pending_fee_update.take().unwrap();
        self.deposit_fee_bps = update.deposit_fee_bps;
        self.claim_fee_bps   = update.claim_fee_bps;
    }

    /// Cancel a pending fee increase. Owner only.
    pub fn cancel_fee_update(&mut self) {
        require!(env::predecessor_account_id() == self.owner_id, "Owner only");
        self.pending_fee_update = None;
    }

    /// Permanently remove the deployer's full-access key.
    /// upgrades_locked is set to true only inside the callback after confirming
    /// the key deletion succeeded, preventing a false "locked" state if the
    /// wrong key is supplied.  IRREVERSIBLE once the callback confirms.
    pub fn lock_upgrades(&mut self, deployer_key: PublicKey) -> Promise {
        require!(env::predecessor_account_id() == self.owner_id, "Owner only");
        require!(!self.upgrades_locked, "Upgrades already locked");
        Promise::new(env::current_account_id())
            .delete_key(deployer_key)
            .then(
                Promise::new(env::current_account_id())
                    .function_call(
                        "on_lock_upgrades_complete".to_string(),
                        vec![],
                        NearToken::from_yoctonear(0),
                        CALLBACK_GAS,
                    )
            )
    }

    /// Callback for lock_upgrades(). #[private] enforced by near-sdk.
    #[private]
    pub fn on_lock_upgrades_complete(&mut self) {
        require!(
            near_sdk::is_promise_success(),
            "Key deletion failed — upgrades NOT locked; verify the correct key was provided"
        );
        self.upgrades_locked = true;
    }

    /// Step 1 of two-step ownership transfer.
    /// v4 fix: rejects self-transfer proposals.
    pub fn propose_ownership(&mut self, new_owner_id: AccountId) {
        require!(env::predecessor_account_id() == self.owner_id, "Owner only");
        require!(
            new_owner_id != self.owner_id,
            "New owner must differ from current owner"
        );
        write_pending_owner(Some(&new_owner_id));
    }

    /// Step 2 of two-step ownership transfer.
    /// Must be called by the pending_owner to take effect.
    pub fn accept_ownership(&mut self) {
        let caller = env::predecessor_account_id();
        let pending = read_pending_owner().expect("No pending ownership transfer");
        require!(caller == pending, "Only the pending owner can accept");
        self.owner_id = pending;
        write_pending_owner(None);
    }

    /// Cancel a pending ownership transfer. Owner only.
    pub fn cancel_ownership_transfer(&mut self) {
        require!(env::predecessor_account_id() == self.owner_id, "Owner only");
        write_pending_owner(None);
    }

    // ── View methods ────────────────────────────────────────────────────────

    pub fn get_account(&self, account_id: AccountId) -> AccountView {
        let d = self.delegators.get(&account_id).unwrap_or_default();
        let staked  = self.amount_for_shares(d.stake_shares);
        let rewards = staked.saturating_sub(d.principal);
        AccountView {
            staked_balance:          U128(staked),
            unstaked_balance:        U128(d.unstaked_balance),
            total_balance:           U128(staked + d.unstaked_balance),
            principal:               U128(d.principal),
            rewards_earned:          U128(rewards),
            can_withdraw:            d.unstaked_balance > 0
                                     && env::epoch_height() >= d.unstake_available_epoch,
            is_locked:               env::block_timestamp() < d.unlock_timestamp_ns,
            unlock_timestamp_ns:     d.unlock_timestamp_ns,
            unstake_available_epoch: d.unstake_available_epoch,
        }
    }

    pub fn get_total_staked_balance(&self) -> U128 { U128(self.total_staked_balance) }
    pub fn get_total_stake_shares(&self)   -> U128 { U128(self.total_stake_shares) }
    pub fn get_owner_id(&self)             -> AccountId { self.owner_id.clone() }
    pub fn is_upgrades_locked(&self)       -> bool { self.upgrades_locked }
    pub fn get_pending_owner(&self)        -> Option<AccountId> { read_pending_owner() }

    /// Returns true if the staking key is present and has the expected byte
    /// length for a supported PQC algorithm (FNDSA, MLDSA, or SLHDSA).
    ///
    /// LENGTH CHECK ONLY — does not validate Borsh encoding or cryptographic
    /// correctness.  A key with the right byte count but wrong format will
    /// pass this check but may produce an invalid stake promise on-chain.
    /// Use as a quick operational sanity check, not a cryptographic guarantee.
    pub fn is_restake_healthy(&self) -> bool {
        let klen = self.staking_key_bytes.len();
        !self.staking_key_bytes.is_empty()
            && (klen == FNDSA_KEY_BORSH_LEN
                || klen == MLDSA_KEY_BORSH_LEN
                || klen == SLHDSA_KEY_BORSH_LEN)
    }

    pub fn get_fees(&self) -> PoolFees {
        PoolFees { deposit_fee_bps: self.deposit_fee_bps, claim_fee_bps: self.claim_fee_bps }
    }

    pub fn get_pending_fee_update(&self) -> Option<PendingFeeUpdate> {
        self.pending_fee_update.clone()
    }

    pub fn get_account_staked_balance(&self, account_id: AccountId) -> U128 {
        let d = self.delegators.get(&account_id).unwrap_or_default();
        U128(self.amount_for_shares(d.stake_shares))
    }

    pub fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128 {
        U128(self.delegators.get(&account_id).map(|d| d.unstaked_balance).unwrap_or(0))
    }

    pub fn get_account_total_balance(&self, account_id: AccountId) -> U128 {
        let d = self.delegators.get(&account_id).unwrap_or_default();
        U128(self.amount_for_shares(d.stake_shares) + d.unstaked_balance)
    }

    // ── Internal ────────────────────────────────────────────────────────────

    fn internal_ping(&mut self) {
        let locked = env::account_locked_balance().as_yoctonear();

        if locked > self.last_locked_balance && self.last_locked_balance > 0 {
            let total_reward = locked - self.last_locked_balance;
            let delegator_reward = muldiv128(
                total_reward,
                self.total_staked_balance,
                self.last_locked_balance,
            ).min(total_reward);
            self.total_staked_balance += delegator_reward;

        } else if locked < self.last_locked_balance && self.last_locked_balance > 0 {
            let decrease = self.last_locked_balance - locked;
            let delegator_decrease = muldiv128(
                decrease,
                self.total_staked_balance,
                self.last_locked_balance,
            );
            self.total_staked_balance =
                self.total_staked_balance.saturating_sub(delegator_decrease);
        }
        // If last_locked_balance == 0: skip attribution (bootstrap / full-unstake).
        // Intentional — panicking here would brick the contract if the validator
        // ever fully unstakes.

        self.last_locked_balance = locked;
    }

    fn internal_restake(&mut self) {
        if self.staking_key_bytes.is_empty() { return; }

        let klen = self.staking_key_bytes.len();
        if klen != FNDSA_KEY_BORSH_LEN
            && klen != MLDSA_KEY_BORSH_LEN
            && klen != SLHDSA_KEY_BORSH_LEN
        {
            // Silent return is intentional: do not panic and brick user operations
            // if the key is temporarily misconfigured.  Call is_restake_healthy()
            // to detect this state without executing a transaction.
            return;
        }

        #[cfg(target_arch = "wasm32")]
        {
            let acct = env::current_account_id().to_string();
            // max() is intentional: prevents accidentally triggering a partial
            // unstake of validator-owned stake when pool balance temporarily dips
            // below last_locked_balance after claims, slashes, or withdrawals.
            let amt  = self.total_staked_balance.max(self.last_locked_balance);
            let pk   = self.staking_key_bytes.clone();
            unsafe {
                let idx = sys_promise_batch_create(&acct);
                sys_promise_batch_action_stake(idx, amt, &pk);
            }
        }
    }

    fn shares_for(&self, amount: u128) -> u128 {
        if self.total_stake_shares == 0 || self.total_staked_balance == 0 {
            return amount;
        }
        self.shares_for_amount(amount)
    }

    fn shares_for_amount(&self, amount: u128) -> u128 {
        if self.total_staked_balance == 0 { return amount; }
        muldiv128(amount, self.total_stake_shares, self.total_staked_balance)
    }

    /// Compute fee shares for the owner after the delegator's burn.
    /// Uses net_reward (not full rewards) as the denominator because the
    /// owner's fee stays inside the pool — only net_reward actually leaves.
    /// Returns `fee` when ps == 0 or ph == 0 (degenerate pool state),
    /// providing a 1:1 fallback rather than silently giving the owner zero.
    fn shares_for_amount_post_reduce(&self, fee: u128, net_reward: u128, burned: u128) -> u128 {
        let ps = self.total_staked_balance.saturating_sub(net_reward);
        let ph = self.total_stake_shares.saturating_sub(burned);
        if ps == 0 || ph == 0 { return fee; }
        muldiv128(fee, ph, ps)
    }

    fn amount_for_shares(&self, shares: u128) -> u128 {
        if self.total_stake_shares == 0 { return 0; }
        muldiv128(shares, self.total_staked_balance, self.total_stake_shares)
    }
}

// ── Overflow-safe multiply-divide ─────────────────────────────────────────────

fn muldiv128(a: u128, b: u128, c: u128) -> u128 {
    if c == 0 { return 0; }
    if let Some(ab) = a.checked_mul(b) {
        return ab / c;
    }
    let q = a / c;
    let r = a % c;
    let term1 = q.saturating_mul(b);
    let term2 = if let Some(rb) = r.checked_mul(b) {
        rb / c
    } else {
        let bq = b / c;
        let br = b % c;
        let t1 = r.saturating_mul(bq);
        let t2 = r.saturating_mul(br) / c;
        t1.saturating_add(t2)
    };
    term1.saturating_add(term2)
}

// ── Key parsing ───────────────────────────────────────────────────────────────

fn parse_key_string(key_str: &str) -> Vec<u8> {
    let colon_pos = key_str.find(':');
    require!(colon_pos.is_some(), "Key format must be 'algo:base58data'");
    let colon = colon_pos.unwrap();

    let algo = &key_str[..colon];
    let b58  = &key_str[colon + 1..];

    let key_type_byte: u8 = match algo {
        "mldsa"  => 2,
        "fndsa"  => 3,
        "slhdsa" => 4,
        _ => env::panic_str("Unknown key algorithm. Expected: fndsa, mldsa, or slhdsa"),
    };

    let decode_result = bs58::decode(b58).into_vec();
    require!(decode_result.is_ok(), "Key data is not valid base58");
    let key_bytes = decode_result.unwrap();

    let expected_len: usize = match algo {
        "mldsa"  => 1952,
        "fndsa"  => 897,
        "slhdsa" => 32,
        _ => env::panic_str("Unreachable: unknown algorithm in length check"),
    };
    require!(
        key_bytes.len() == expected_len,
        "Key length does not match expected size for this algorithm"
    );

    let mut result = Vec::with_capacity(1 + 4 + key_bytes.len());
    result.push(key_type_byte);
    let len_le = (key_bytes.len() as u32).to_le_bytes();
    result.extend_from_slice(&len_le);
    result.extend_from_slice(&key_bytes);
    result
}
