/*!
 * Final Layer Staking Pool Contract v2 — Security Hardened
 * (near-sdk 5.x, legacy collections)
 *
 * v2 changes (from v11):
 *   - Removed pause() / unpause() — eliminates fund-locking attack vector
 *   - Removed fix_pool_accounting(), fix_delegator_shares(), fix_delegator_unstaked()
 *     — eliminates balance manipulation by pool owner
 *   - Removed debug_get_stake_shares(), debug_amount_for_shares()
 *   - Hard cap claim fee at 10% (MAX_CLAIM_FEE_BPS = 1000, was 10000)
 *   - Fee increases require 48h timelock via propose_fee_update() / execute_fee_update()
 *   - Fee decreases are immediate (always user-favorable)
 *   - Added lock_upgrades(): owner calls once to delete their full-access key,
 *     making the contract permanently non-upgradeable
 *   - Added staking_key_bytes length validation in internal_restake() (H1 fix)
 *   - Replaced .expect()/panic!() in parse_key_string with require!() (M2 fix)
 *   - deposit_and_stake() gains optional min_shares_out slippage guard (M3 fix)
 *   - Fee calculations use muldiv128() to prevent hypothetical overflow (M4 fix)
 *   - muldiv128 division-by-zero "fix" NOT applied — audit finding incorrect,
 *     existing guard covers all paths
 */

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near, require, AccountId, NearToken, Promise, PublicKey};

// ── Constants ─────────────────────────────────────────────────────────────────

const LOCKUP_NS: u64             = 4 * 43_200 * 1_000_000_000;
const NUM_EPOCHS_TO_UNLOCK: u64  = 4;
const MAX_DEPOSIT_FEE_BPS: u16   = 10;    // 0.1% hard cap
const MAX_CLAIM_FEE_BPS: u16     = 1000;  // 10% hard cap (was 10000 / 100% in v11)
const MIN_STAKE: u128            = 1_000_000_000_000_000_000_000_000; // 1 FLC
const FEE_TIMELOCK_NS: u64       = 48 * 3_600 * 1_000_000_000; // 48h

// Expected Borsh-encoded byte lengths for each supported PQC key type.
// Used in internal_restake() to validate key bytes before the unsafe host call.
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

// ── Contract ──────────────────────────────────────────────────────────────────

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
}

impl Default for StakingPool {
    fn default() -> Self {
        panic!("StakingPool must be initialized via new()")
    }
}

#[near]
impl StakingPool {

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
    // NOTE: pause() has been removed. Unstake and withdraw are always available.

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
        let actual = if self.total_stake_shares > 0 {
            shares as u128 * self.total_staked_balance / self.total_stake_shares
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

    pub fn withdraw_all(&mut self) {
        let account_id = env::predecessor_account_id();
        let mut d = self.delegators.get(&account_id).expect("No withdrawal found");
        require!(d.unstaked_balance > 0, "No unstaked balance");
        require!(
            env::epoch_height() >= d.unstake_available_epoch,
            "Still in unbonding period"
        );
        let amount = d.unstaked_balance;
        d.unstaked_balance = 0;
        d.unstake_available_epoch = 0;
        self.delegators.insert(&account_id, &d);
        near_sdk::Promise::new(account_id)
            .transfer(NearToken::from_yoctonear(amount))
            .detach();
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
            let fee_shares = self.shares_for_amount_post_reduce(fee, rewards, burn);
            let owner_id = self.owner_id.clone();
            let mut od = self.delegators.get(&owner_id).unwrap_or_default();
            od.stake_shares += fee_shares;
            od.principal += fee;
            self.delegators.insert(&owner_id, &od);
            self.total_stake_shares = self.total_stake_shares
                .saturating_sub(burn)
                .saturating_add(fee_shares);
            self.total_staked_balance = self.total_staked_balance.saturating_sub(net_reward);
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

        d.principal = staked;
        d.unlock_timestamp_ns = env::block_timestamp() + LOCKUP_NS;
        self.delegators.insert(&account_id, &d);
    }

    pub fn sync_principal(&mut self) {
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
    /// Fee DECREASES take effect immediately.
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
    /// Anyone can call this to prevent owner from blocking the scheduled update.
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

    /// Permanently remove the deployer's full-access key, making this contract
    /// non-upgradeable. Can only be called once. IRREVERSIBLE.
    pub fn lock_upgrades(&mut self, deployer_key: PublicKey) {
        require!(env::predecessor_account_id() == self.owner_id, "Owner only");
        require!(!self.upgrades_locked, "Upgrades already locked");
        self.upgrades_locked = true;
        Promise::new(env::current_account_id()).delete_key(deployer_key);
    }

    pub fn transfer_ownership(&mut self, new_owner_id: AccountId) {
        require!(env::predecessor_account_id() == self.owner_id, "Owner only");
        self.owner_id = new_owner_id;
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

        self.last_locked_balance = locked;
    }

    fn internal_restake(&mut self) {
        if self.staking_key_bytes.is_empty() { return; }

        // v2 H1 fix: validate key bytes length before unsafe host call.
        let klen = self.staking_key_bytes.len();
        if klen != FNDSA_KEY_BORSH_LEN
            && klen != MLDSA_KEY_BORSH_LEN
            && klen != SLHDSA_KEY_BORSH_LEN
        {
            return;
        }

        #[cfg(target_arch = "wasm32")]
        {
            let acct = env::current_account_id().to_string();
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

    fn shares_for_amount_post_reduce(&self, fee: u128, rewards: u128, burned: u128) -> u128 {
        let ps = self.total_staked_balance.saturating_sub(rewards);
        let ph = self.total_stake_shares.saturating_sub(burned);
        if ps == 0 { return fee; }
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
        _ => {
            require!(false,
                "Unknown key algorithm. Expected: fndsa, mldsa, or slhdsa");
            unreachable!()
        }
    };

    let decode_result = bs58::decode(b58).into_vec();
    require!(decode_result.is_ok(), "Key data is not valid base58");
    let key_bytes = decode_result.unwrap();

    let expected_len: usize = match algo {
        "mldsa"  => 1952,
        "fndsa"  => 897,
        "slhdsa" => 32,
        _        => key_bytes.len(),
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
