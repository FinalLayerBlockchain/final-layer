# PQC Gas Constant Rationale

## Overview

Gas in Final Layer (inherited from NEAR Protocol) represents the computational budget for executing operations. For PQC signature verification, gas must be set so that:

1. A min-spec validator can keep up with block production even under adversarial load
2. Users are not overcharged relative to actual compute cost
3. Underpriced operations cannot be used as a DoS vector

---

## Benchmark Methodology

Gas constants were calibrated using a **1000-iteration benchmark** of each PQC verify function, run on two real validator hardware profiles:

**Machine A (4-core, 16GB RAM):** Primary production validator  
**Machine B (2-core, 4GB RAM):** Secondary validator — used as **min-spec reference**

Gas constants are set based on **Machine B (min-spec)** at the **p99** percentile. This ensures that even the weakest validator in the network can handle worst-case verification workloads without falling behind.

The benchmark used the native `near-crypto` Rust crate compiled with release optimizations — the same code path as the actual node.

---

## Benchmark Results

### Machine A (4-core, 16GB)

| Algorithm | p50 | p95 | p99 |
|---|---|---|---|
| FN-DSA | 0.041ms | 0.148ms | 0.192ms |
| ML-DSA | 0.162ms | 0.521ms | 0.894ms |
| SLH-DSA | 0.626ms | 1.847ms | 2.203ms |

### Machine B (2-core, 4GB) — Min-Spec Reference

| Algorithm | p50 | p95 | p99 |
|---|---|---|---|
| FN-DSA | 0.055ms | 0.187ms | 0.241ms |
| ML-DSA | 0.330ms | 1.124ms | 1.703ms |
| SLH-DSA | 0.972ms | 3.614ms | 5.098ms |

---

## Gas Derivation

NEAR's gas model: **1 TGas ≈ 1ms** of compute on a reference validator.

Using min-spec p99 values:

| Algorithm | p99 (min-spec) | Theoretical min | Assigned gas | Headroom |
|---|---|---|---|---|
| FN-DSA | 0.241ms | 0.241 TGas | **1.4 TGas** | 5.8x |
| ML-DSA | 1.703ms | 1.703 TGas | **3.0 TGas** | 1.76x |
| SLH-DSA | 5.098ms | 5.098 TGas | **8.0 TGas** | 1.57x |

### Why different headroom multipliers?

**FN-DSA gets the largest headroom (5.8x)** because:
- It is the recommended default algorithm for most users
- Generous headroom ensures it stays cheap even on future slower hardware
- Small signatures (666B) mean no chunk-size concerns

**ML-DSA and SLH-DSA get tighter headroom (~1.6–1.8x)** because:
- These are power-user/institutional algorithms
- Tighter headroom keeps them accessible (not prohibitively expensive)
- Minimum safe headroom over p99 is ~1.5x to absorb CPU scheduling jitter

---

## Why SLH-DSA Has the Highest Gas (+150% from initial estimate)

SLH-DSA is expensive for two compounding reasons:

**1. CPU cost** — SPHINCS+ verification requires hashing through a hypertree structure. Unlike lattice schemes (FN-DSA, ML-DSA) that use polynomial arithmetic, SPHINCS+ chains many SHA-2/SHAKE operations. At p99 on min-spec: **5.098ms** — over 5× slower than FN-DSA.

**2. Bandwidth cost** — SLH-DSA signatures are ~8,000 bytes. In NEAR's chunked architecture, each chunk has a hard size limit. An 8KB signature consumes ~12× more chunk space than an FN-DSA signature (666B), reducing effective TX throughput.

**Combined attack surface:** Without correct gas pricing, an attacker could submit SLH-DSA TXs that are cheap to create but expensive to verify, exceeding one block time on min-spec validators — a structural DoS vector. At 8.0 TGas, this vector is closed.

---

## Protocol Version History

| Version | Change |
|---|---|
| v1001 | Genesis — FN-DSA 1.4 TGas, ML-DSA 2.1 TGas, SLH-DSA 3.2 TGas |
| v1002 | 9-shard deployment (no gas change) |
| v1003 | **Gas rebalance hard fork**: ML-DSA → 3.0 TGas, SLH-DSA → 8.0 TGas |

Gas constant changes are consensus-critical (all validators must agree on gas costs → requires hard fork).
