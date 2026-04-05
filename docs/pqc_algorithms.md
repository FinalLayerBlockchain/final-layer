# PQC Algorithm Reference

## Why These Three Algorithms?

NIST finalized three post-quantum digital signature standards in August 2024. Final Layer implements all three, giving users and validators a choice based on their performance and size requirements.

---

## FN-DSA (Falcon-512) — FIPS 206

**Recommended for most users.**

| Property | Value |
|---|---|
| Standard | FIPS 206 |
| Family | Lattice (NTRU lattices) |
| Security level | NIST Level 1 (equivalent to AES-128) |
| Public key size | 897 bytes |
| Signature size | 666 bytes |
| Verification time (p99) | 0.24ms (2-core validator) |
| Gas cost | 1.4 TGas |

**Strengths:** Smallest signatures of the three schemes. Fast verification. Compact on-chain footprint.

**Tradeoffs:** Signing is stateful in some implementations; Final Layer uses the stateless variant.

---

## ML-DSA (Dilithium3) — FIPS 204

**Recommended for institutional validators.**

| Property | Value |
|---|---|
| Standard | FIPS 204 |
| Family | Lattice (Module-LWE) |
| Security level | NIST Level 3 (equivalent to AES-192) |
| Public key size | 1952 bytes |
| Signature size | 3309 bytes |
| Verification time (p99) | 1.70ms (2-core validator) |
| Gas cost | 3.0 TGas |

**Strengths:** Higher security level than FN-DSA. Simple, well-analyzed construction. Good software performance.

**Tradeoffs:** Larger keys and signatures than FN-DSA. Higher gas cost.

---

## SLH-DSA (SPHINCS+-128) — FIPS 205

**For maximum long-term security guarantees.**

| Property | Value |
|---|---|
| Standard | FIPS 205 |
| Family | Hash-based (Merkle hypertree) |
| Security level | NIST Level 1 (equivalent to AES-128) |
| Public key size | 32 bytes |
| Signature size | ~8000 bytes |
| Verification time (p99) | 5.10ms (2-core validator) |
| Gas cost | 8.0 TGas |

**Strengths:** Security based entirely on hash function security — no lattice assumptions. Smallest public key. Most conservative security argument.

**Tradeoffs:** Very large signatures (~8KB) consume significant chunk space. Slowest verification. Highest gas cost.

---

## Key Encoding

All keys use base58 encoding with an algorithm prefix:

```
fndsa:<base58(bytes)>
mldsa:<base58(bytes)>
slhdsa:<base58(bytes)>
```

When stored in Borsh-serialized format (e.g., inside account state), keys are prefixed with a 4-byte little-endian u32 length value, matching the standard `Vec<u8>` Borsh encoding.

### Key size validation

`parse_key_string()` in the staking pool enforces exact byte lengths:

```rust
let pk_len: usize = match algo {
    "mldsa"  => 1952,
    "fndsa"  => 897,
    "slhdsa" => 32,
    _        => key_bytes.len(),
};
if key_bytes.len() != pk_len {
    panic!("Key must be exactly {} bytes for {}, got {}", pk_len, algo, key_bytes.len());
}
```

This prevents silent truncation of malformed keys.

---

## Comparison with Ed25519 (removed)

| | Ed25519 | FN-DSA | ML-DSA | SLH-DSA |
|---|---|---|---|---|
| Quantum-safe | **No** | Yes | Yes | Yes |
| PK size | 32 bytes | 897 bytes | 1952 bytes | 32 bytes |
| Sig size | 64 bytes | 666 bytes | 3309 bytes | ~8000 bytes |
| Verify time | ~0.05ms | ~0.24ms | ~1.70ms | ~5.10ms |

Ed25519 is broken by Shor's algorithm on a cryptographically relevant quantum computer. Final Layer removes it entirely — there is no fallback to classical crypto.
