# Architecture

Final Layer runs on NEAR Protocol's sharded proof-of-stake design with post-quantum cryptography replacing the classical key layer.

## Overview

The network runs 9 shards in parallel. Validators are assigned to shards by stake weight and rotate each epoch (43,200 blocks, roughly 12 hours). Consensus uses NEAR's Doomslug protocol — blocks finalize in 1-2 seconds under normal conditions, requiring endorsement from 2/3+ of stake.

Validator signing keys use FN-DSA or ML-DSA instead of Ed25519.

## Key types

The `KeyType` enum in `near-crypto`:

```rust
pub enum KeyType {
    MLDSA  = 2,
    FNDSA  = 3,
    SLHDSA = 4,
}
```

Ed25519 (0) and secp256k1 (1) are removed. All keys are encoded as `algo:base58(bytes)`. In Borsh-serialized storage (account state, transaction payloads) keys carry a 4-byte little-endian u32 length prefix matching the standard `Vec<u8>` encoding. This matters — early versions of the staking contract omitted this prefix and caused silent key corruption.

## PQC host functions

Three verification functions are available to WASM contracts:

```
pqc_verify_fndsa  (pk, pk_len, sig, sig_len, msg, msg_len) -> u64
pqc_verify_mldsa  (pk, pk_len, sig, sig_len, msg, msg_len) -> u64
pqc_verify_slhdsa (pk, pk_len, sig, sig_len, msg, msg_len) -> u64
```

Each deducts gas before executing. The gas cost reflects p99 verification time on a 2-core 4GB validator with a safety multiplier.

## Staking

Delegators deposit FLC into a validator's staking pool contract. The pool tracks each delegator's staked balance, a 48-hour unlock timestamp, and a 4-epoch unbonding window. Withdrawals are gated by both checks — you can't withdraw until both the timestamp and the epoch condition are met.

```
Delegator -> deposit_and_stake() -> staking pool -> validator account -> chain
```

## Client stack

The wallet is a Next.js app. The block explorer is a Node.js indexer backed by SQLite. Both talk to nodes over the standard NEAR JSON-RPC API on port 3030.

## Validator requirements

Minimum: 4 vCPU, 8GB RAM, 200GB SSD. Open ports 3030 (RPC) and 24567 (P2P).
