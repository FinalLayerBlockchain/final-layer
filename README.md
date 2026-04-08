# Final Layer — Staking

Final Layer is a quantum-resistant proof-of-stake blockchain. This repository contains the staking pool smart contract (v10), staking documentation, and benchmarks.

Full node source: [nearcore-pq](https://github.com/FinalLayerBlockchain/nearcore-pq).  
Chain ID: `final-layer-mainnet` · Protocol v1005 · 9 shards · ~1 second blocks · Native token: **FLC**

---

## How staking secures the network

Final Layer uses NEAR Protocol's sharded Doomslug consensus. Validators lock FLC as economic stake — if a validator acts dishonestly or goes offline, their locked balance is at risk. The more FLC locked across validators, the more capital an attacker would need to compromise the network.

- **9 shards** process transactions in parallel. Each shard is assigned a subset of validators per epoch.
- **Validators** produce blocks and chunks, earning block rewards funded by protocol inflation.
- **Delegators** stake FLC to a validator pool and share in those rewards proportionally — without running a node.
- **Epoch** (~12 hours, 43,200 blocks): the cycle at which validator assignments rotate, rewards are distributed, and staking changes take effect.

The more FLC staked, the more expensive it is for any party to accumulate enough influence to attack a shard. Staking is the primary mechanism that translates token supply into network security.

---

## Staking pool contract

Each validator runs a staking pool contract deployed to their validator account (e.g. `validator-1.fl`, `king.fl`). Delegators interact with this contract directly — the validator never touches delegator funds.

**Current version: v10** (`fl_staking_pool`)

### Key parameters

| Parameter | Value |
|---|---|
| Base APY | 20% (halving schedule applies) |
| Epoch length | 43,200 blocks (~12 hours) |
| Lockup period | 48 hours after any deposit or compound |
| Unbonding period | 4 epochs (~48 hours) after unstaking |
| Deposit fee | 0.05% (5 bps), paid once on entry |
| Claim fee | 5% (500 bps), deducted from claimed rewards |
| Compound fee | None — compounding is always free |
| Minimum stake | 1 FLC |

### How the pool tracks balances

The pool uses a **share-price model**. When you deposit, you receive shares representing your fraction of the pool. As the pool earns block rewards, the value of each share increases. Your effective FLC balance is always:

```
share_price  = total_staked_balance / total_shares
your_balance = your_shares × share_price
```

This means rewards are automatically reflected in your balance every epoch — no claim transaction required to accumulate them.

### Proportional reward attribution (v10)

Prior to v10, validator self-stake yield could inadvertently leak to delegators. v10 fixes this: when the protocol reports new locked balance (block rewards arrived), the pool credits only the delegator-proportional fraction:

```
delegator_reward = total_reward × (total_staked / last_locked)
```

Validators earn on their own stake; delegators earn exactly their proportional share of the pool — neither more nor less. This is enforced in `internal_ping()`, which runs at the start of every state-changing call.

---

## Staking lifecycle

### 1. Deposit and stake

Send FLC to a validator pool. The 0.1% deposit fee is deducted before shares are minted. A 48-hour lock starts immediately.

```bash
fl-send-tx \
  --receiver validator-1.fl \
  --method deposit_and_stake \
  --deposit 1000 \
  --key-file mykey.json
```

After this call your balance grows each epoch as the pool earns rewards. No further action is needed to accumulate.

### 2. View your balance

```bash
fl-send-tx \
  --receiver validator-1.fl \
  --method get_account \
  --args '{"account_id": "YOUR_ACCOUNT_ID"}' \
  --key-file mykey.json
```

Example response:
```json
{
  "staked_balance": "1002150000000000000000000000",
  "unstaked_balance": "0",
  "is_locked": false
}
```

`staked_balance` already includes all accrued rewards. The difference from your original deposit is what you have earned so far.

### 3. Claim rewards

Moves earned rewards (minus the 0.1% claim fee) to your wallet. Requires the 48-hour lock to have expired. Resets the 48-hour lock on your position.

```bash
fl-send-tx \
  --receiver validator-1.fl \
  --method claim_rewards \
  --key-file mykey.json
```

### 4. Compound rewards (recommended)

Reinvests accrued rewards back into staked principal — no fee, atomic, gas-efficient. Resets the 48-hour lock.

```bash
fl-send-tx \
  --receiver validator-1.fl \
  --method compound \
  --key-file mykey.json
```

Compounding is more efficient than claim + re-deposit: it costs less gas and avoids paying the 0.1% deposit fee on the reinvested amount.

### 5. Unstake

Moves FLC from staked to unstaked balance. Requires the 48-hour lock to have expired. Starts a 4-epoch (~48 hour) unbonding timer. Unstaked balance earns no further rewards.

```bash
# Unstake a specific amount (value in yoctoFLC, 1 FLC = 10^24 yoctoFLC)
fl-send-tx \
  --receiver validator-1.fl \
  --method unstake \
  --args '{"amount": "500000000000000000000000000"}' \
  --key-file mykey.json

# Or unstake your entire position
fl-send-tx \
  --receiver validator-1.fl \
  --method unstake_all \
  --key-file mykey.json
```

### 6. Withdraw

After 4 full epochs have passed since unstaking, withdraw your FLC back to your wallet.

```bash
fl-send-tx \
  --receiver validator-1.fl \
  --method withdraw_all \
  --key-file mykey.json
```

### Full lifecycle timeline

```
Day 0    deposit_and_stake(1000 FLC)
         └─ 48h lock starts, shares minted

Day 2+   lock expires

Day 2    compound()     ← free, no fee, resets lock to Day 4
Day 4    compound()     ← stack rewards, resets lock to Day 6

Day N    unstake_all()
         └─ 4-epoch unbonding starts (~48h)

Day N+2  withdraw_all() ← FLC returned to wallet
```

---

## Rewards

### How rewards accumulate

Each epoch, the protocol distributes block rewards to validators based on their total locked balance. The staking pool captures this by comparing `env::account_locked_balance()` to the previous epoch's recorded locked balance — the delta is the epoch reward. This happens inside `internal_ping()`, which runs automatically at the start of every deposit, compound, claim, unstake, or withdraw call.

**You do not need to do anything between epochs.** Your `staked_balance` grows silently in the background.

### Why your reward may show 0 right after staking

The pool only updates when someone calls it. If no transaction has landed on the pool since the last epoch boundary, the internal share price has not yet been updated to reflect the new rewards. As soon as anyone calls the pool (or you call `ping` manually), the balance catches up.

To force an update without changing your position:

```bash
fl-send-tx \
  --receiver validator-1.fl \
  --method ping \
  --key-file mykey.json
```

### APY and halving

The starting APY is 20%. The protocol follows a halving schedule — reward rates reduce over time as the network matures, following the same model as Bitcoin halvings. The current effective APY is displayed on the staking dashboard at `/wallet/staking`.

### Fee summary

| Action | Fee | Recipient |
|---|---|---|
| `deposit_and_stake` | 0.05% of deposit | Validator |
| `claim_rewards` | 5% of rewards claimed | Validator |
| `compound` | None | — |
| `unstake` / `unstake_all` | None | — |
| `withdraw_all` | None | — |

---

## Active validator pools

| Pool | Account |
|---|---|
| Validator 1 | `validator-1.fl` |
| Validator 2 | `validator-2.fl` |

Query pool totals:

```bash
# Total FLC staked in a pool
fl-send-tx --receiver king.fl --method get_total_staked_balance --args '{}'

# Total shares outstanding
fl-send-tx --receiver king.fl --method get_total_stake_shares --args '{}'
```

---

## PQC key registration

Validators register their block-signing key with the pool via `update_staking_key()`. Final Layer supports three NIST post-quantum standards:

| Algorithm | Standard | Public key | Signature | Gas |
|---|---|---|---|---|
| FN-DSA (Falcon-512) | FIPS 206 | 897 bytes | 666 bytes | 1.4 TGas |
| ML-DSA (Dilithium3) | FIPS 204 | 1952 bytes | 3309 bytes | 3.0 TGas |
| SLH-DSA (SPHINCS+-128) | FIPS 205 | 32 bytes | ~8000 bytes | 8.0 TGas |

FN-DSA is the recommended default: smallest keys, lowest gas, and sufficient security for all current threat models. SLH-DSA (hash-based, no lattice assumptions) is available for maximum long-term security at higher gas cost.

Key lengths are validated on-chain with exact-byte checks — wrong length causes an explicit on-chain panic. No silent truncation or padding.

---

## Security properties

| Property | Detail |
|---|---|
| Quantum resistance | All signing keys are NIST-standardized PQC. Ed25519 and secp256k1 are not accepted. |
| Non-custodial | The pool contract never holds validator private keys. Delegator funds are locked under delegator-controlled accounts. |
| 48-hour lockup | Prevents flash-deposit attacks: an attacker cannot stake, capture an epoch reward, and immediately exit within a single epoch. |
| 4-epoch unbonding | After unstaking, FLC stays locked for ~48 hours. This window allows protocol-level slashing logic to run before capital can exit. |
| Proportional attribution | v10 mathematical proof: each party earns exactly on their own contributed stake. No yield sharing between validator self-stake and delegator positions. |
| Share-price model | Rewards are embedded in share price, not in separate reward buckets. A delegator cannot be credited for rewards earned before they joined the pool. |

---

## Repository layout

```
contracts/staking-pool/src/lib.rs      Staking pool contract (v10)
docs/staking.md                        Full lifecycle and parameter reference
docs/staking-pool-bug-history.md       All 10 contract versions and every bug fixed
docs/architecture.md                   System architecture
docs/pqc_algorithms.md                 Algorithm comparison and encoding details
docs/gas_rationale.md                  How gas constants were derived
docs/benchmarks.md                     Benchmark results across two hardware profiles
docs/protocol_upgrades.md              Hard fork procedure
```

## License

Apache 2.0, inherited from NEAR Protocol.
