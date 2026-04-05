# Protocol Upgrades

## When a hard fork is needed

Gas constants, new host functions, and changes to state transition logic are consensus-critical. Every validator must agree on the cost of every operation. If two validators run different gas constants they'll produce different state roots for the same block and the chain splits. So any change to these values requires a coordinated upgrade.

## Procedure

Patch the source. For gas constant changes, edit `runtime/near-vm-runner/src/logic/pqc_host_fns.rs`. Bump `STABLE_PROTOCOL_VERSION` in `core/primitives-core/src/version.rs`.

Build the new binary:
```bash
cargo build --release -p neard
```

Distribute to all validators before restarting any of them. Use `install` rather than `cp` to avoid file-busy errors on a running binary:
```bash
install -m 755 /path/to/new/neard /usr/local/bin/neard
```

Stop all validators, then restart them together:
```bash
systemctl stop fl-node
# wait for all nodes to confirm stopped
systemctl start fl-node
```

Verify they're all active and syncing. The chain protocol version upgrades automatically at the next epoch boundary once 2/3+ of stake has voted for the new version. The on-chain protocol version will show the old number until that boundary — this is expected.

## Version history

v1001: Genesis. PQC cryptography, 9-shard config.
v1002: Multi-shard epoch config (`epoch_configs/1002.json`).
v1003: Gas rebalance. ML-DSA raised from 2.1 to 3.0 TGas, SLH-DSA raised from 3.2 to 8.0 TGas.

## Rollback

Before any upgrade, back up the current binary:
```bash
cp /usr/local/bin/neard /usr/local/bin/neard_backup
```

To roll back: restore the binary and restart.
