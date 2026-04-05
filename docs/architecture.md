# Final Layer Architecture

## High-Level Overview

Final Layer is a sharded proof-of-stake blockchain built on NEAR Protocol's architecture, with post-quantum cryptography replacing all classical elliptic curve schemes.

```
┌─────────────────────────────────────────────────────┐
│                   Client Layer                       │
│   Wallet (Next.js)    │    Block Explorer (Node.js)  │
└───────────┬───────────┴──────────────┬──────────────┘
            │ JSON-RPC                 │ JSON-RPC
┌───────────▼──────────────────────────▼──────────────┐
│                  Final Layer Node (neard)             │
│                                                       │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │  Consensus  │  │  NEAR VM     │  │  near-crypto │ │
│  │  (Doomslug) │  │  (WASM)      │  │  (PQC keys) │ │
│  └─────────────┘  └──────┬───────┘  └─────────────┘ │
│                           │ host functions             │
│                    ┌──────▼───────┐                   │
│                    │ pqc_host_fns │                   │
│                    │ (FN/ML/SLH)  │                   │
│                    └──────────────┘                   │
└───────────────────────────────────────────────────────┘
            │ Sharded State
┌───────────▼───────────────────────────────────────────┐
│                    9 Shards                            │
│  [0] [1] [2] [3] [4] [5] [6] [7] [8]                 │
└───────────────────────────────────────────────────────┘
```

---

## Sharding Configuration

Final Layer uses **9 shards** for parallel transaction processing, configured via `epoch_configs/1002.json`. Validators are assigned to shards based on their stake weight — a validator with ~60% of stake covers ~3 shards, a validator with ~20% covers ~1-2 shards.

Shard assignment rotates each epoch (43,200 blocks ≈ 12 hours).

---

## Consensus: Doomslug

Inherited from NEAR Protocol. Doomslug is a single-round BFT-like finality gadget:
- Blocks are produced by a rotating set of block producers
- Finality requires 2/3+ of validators to endorse a block
- Under normal conditions, blocks finalize in 1-2 seconds

Validator signing keys use **FN-DSA** (Falcon-512) or **ML-DSA** (Dilithium3) instead of Ed25519.

---

## Cryptographic Layer

### Key Types (KeyType enum)

```rust
pub enum KeyType {
    MLDSA  = 2,   // ML-DSA (Dilithium3), FIPS 204
    FNDSA  = 3,   // FN-DSA (Falcon-512), FIPS 206
    SLHDSA = 4,   // SLH-DSA (SPHINCS+-128), FIPS 205
}
```

Ed25519 (`KeyType = 0`) and secp256k1 (`KeyType = 1`) are removed.

### Borsh Serialization

PQC public keys are variable-length and require Borsh's `Vec<u8>` encoding — a 4-byte little-endian u32 length prefix followed by raw key bytes:

```
[len_lo, len_mid1, len_mid2, len_hi, key_byte_0, key_byte_1, ...]
```

This is critical for deserialization in the WASM VM. Early staking pool versions (v1–v2) omitted this prefix, causing silent key corruption.

---

## VM Host Functions

Three PQC verification functions are exposed to WASM contracts:

```
pqc_verify_fndsa  (pk, pk_len, sig, sig_len, msg, msg_len) -> 1 (valid) or 0 (invalid)
pqc_verify_mldsa  (pk, pk_len, sig, sig_len, msg, msg_len) -> 1 or 0
pqc_verify_slhdsa (pk, pk_len, sig, sig_len, msg, msg_len) -> 1 or 0
```

Each call deducts gas before executing. The gas cost covers the expected CPU time at p99 on a min-spec validator, with a safety multiplier.

---

## Staking Architecture

```
Delegator wallet
      │
      │ deposit_and_stake()
      ▼
┌─────────────────────────────────┐
│   fl_staking_pool v5             │
│                                   │
│  staked_balance                   │
│  unstaked_balance                 │
│  rewards_earned                   │
│  unlock_timestamp_ns (48h lock)   │
│  unstake_available_epoch (+4)     │
└──────────────┬────────────────────┘
               │ delegates stake to
               ▼
      Validator account
      (king.fl / validator-1.fl / validator-2.fl)
               │
               │ block production + signing (FN-DSA / ML-DSA)
               ▼
         Final Layer Chain
```

---

## Client Stack

### Wallet
- **Framework:** Next.js (React)
- **Key operations:** Browser-side, no server-side key storage
- **Import format:** Combined PK||SK (`fndsa:<base58>`)
- **QR support:** `qrcode` library for address sharing

### Block Explorer
- **Backend:** Node.js with SQLite indexer
- **Indexer:** Follows chain tip, stores blocks/transactions/accounts
- **Frontend:** Server-rendered explorer UI

Both connect to nodes via the standard NEAR JSON-RPC API on port 3030.

---

## Validator Infrastructure

A minimal Final Layer network requires:
- **1+ block producers** with staked FLC
- **neard** binary built from this fork
- **fl_staking_pool v5** deployed to each validator account
- Firewall: port 3030 (RPC), port 24567 (P2P)

Recommended minimum hardware per validator: 4 vCPU, 8GB RAM, 200GB SSD.
