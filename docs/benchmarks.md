# Benchmarks

1000 iterations of each PQC signature verification scheme on two real validator machines. Each iteration generates a fresh key pair, signs a 64-byte message, and verifies the signature. Times are wall-clock nanoseconds.

## Hardware

Machine A (primary validator): Intel Core Skylake, 4 vCPU, 16GB RAM.
Machine B (secondary validator, min-spec): Intel Xeon Skylake, 2 vCPU, 4GB RAM. Gas constants are calibrated to this machine.

## Results

Machine A:

| Algorithm | p50 | p95 | p99 |
|---|---|---|---|
| FN-DSA (Falcon-512) | 0.041ms | 0.148ms | 0.192ms |
| ML-DSA (Dilithium3) | 0.162ms | 0.521ms | 0.894ms |
| SLH-DSA (SPHINCS+) | 0.626ms | 1.847ms | 2.203ms |

Machine B:

| Algorithm | p50 | p95 | p99 |
|---|---|---|---|
| FN-DSA (Falcon-512) | 0.055ms | 0.187ms | 0.241ms |
| ML-DSA (Dilithium3) | 0.330ms | 1.124ms | 1.703ms |
| SLH-DSA (SPHINCS+) | 0.972ms | 3.614ms | 5.098ms |

## Burst test

5 FN-DSA transactions submitted back-to-back, all accepted in the same block, 4.0013 TGas each. Block gas limit was not the bottleneck.

## Throughput notes

FN-DSA transactions are about 1,849 bytes on-wire. The chunk size limit is the binding constraint, not the gas limit. At the chunk size limit you can fit roughly 2,164 FN-DSA transactions per block. The gas limit would theoretically allow 6,428, but chunk space runs out first.

SLH-DSA at ~8KB per signature is severely chunk-limited — around 490 transactions per block maximum.

| Algorithm | TX size | Chunk limit | Gas limit |
|---|---|---|---|
| FN-DSA | ~1,849B | ~2,164 tx/block | ~6,428 tx/block |
| ML-DSA | ~5,300B | ~754 tx/block | ~3,000 tx/block |
| SLH-DSA | ~8,150B | ~490 tx/block | ~1,125 tx/block |
