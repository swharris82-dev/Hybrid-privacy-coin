# Hybrid Privacy Coin: Zebra Fork

This branch (`hybrid-privacy`) forks the Zcash Foundation's Zebra node to build a
privacy coin that is shielded by default like Monero and uses Zcash's Halo 2 proofs
and viewing keys.

Design spec: the "Hybrid Privacy Coin: Design Spec" doc.

## Status

| Phase 2 item | Status |
| --- | --- |
| Shielded-only consensus rule | Done, tested |
| Node config for a shielded-only network | Done, tested |
| Startup guard against transparent coinbase payments | Done, tested |
| Own genesis block | Done, minted and tested |
| Mining shielded blocks | Done, blocks 1 to 4 mined on a live node |
| Miner address guard | Done, tested |
| RandomX proof of work | Not started |
| Dandelion++ transaction relay | Not started |
| Emission curve, fee burn, tail emission | Not started (reuse NU7, see below) |
| Wallet and light wallet server | Not started |
| Independent audit | Not started |

## What changed

**Shielded-only rule.** A new per-network parameter, `shielded_only_height`. From that
height, every transaction, in blocks and in the mempool, must not:

- spend a transparent output,
- create a transparent output (coinbase included),
- contain Sprout JoinSplits, or
- contain Sapling spends or outputs.

That leaves only the Orchard and Ironwood pools. From NU6.3, Zcash freezes new inflows
to Orchard, so on this chain all value lives in Ironwood. Every transaction hides
sender, receiver, and amount inside a Halo 2 proof, with no trusted setup. Viewing
keys still let a holder disclose their own history.

Mainnet and the public Zcash Testnet never enable this rule.

**Startup guard.** Funding streams and NU6.1 lockbox disbursements pay transparent
addresses. A shielded-only network configured with either refuses to start, instead
of producing blocks nobody can mine.

**Config.** `hybrid/hybrid-testnet.toml` defines a private network with the rule
active from block 1.

**Genesis block.** `custom_genesis_block` in `zebra-chain/src/block/genesis.rs` builds the
first block of a custom network, and the `hybrid-genesis` example wraps it as a command.
The block keeps Zcash's genesis coinbase input, which Zebra recognizes as height 0, and puts
our chain's message in a zero-value `OP_RETURN` output that can never be spent.

| Genesis field | Value |
| --- | --- |
| Hash | `b82107d003c12b71f5c3c12503a087d91516feb0844b99bbf2ae94380a9f86c3` |
| Time | 1791936000 (2026-10-14 00:00 UTC) |
| Difficulty bits | `200f0f0f` (the network's limit) |
| Message | `HybridTestnet genesis 2026-10-14: private by default` |
| File | `hybrid/genesis-block.hex` |

The nonce and Equihash solution are all zeros, so this genesis block only works while
`disable_pow = true`. Re-mint it when the proof of work is final; every node and the config
must then switch to the new hash.

Files touched:

- `zebra-chain/src/parameters/network.rs`: `Network::shielded_only_height`, `Network::is_shielded_only`
- `zebra-chain/src/parameters/network/testnet.rs`: parameter, builder methods, Regtest support, startup guard
- `zebra-chain/src/parameters/network/error.rs`: `ShieldedOnlyTransparentCoinbaseOutputs`
- `zebra-consensus/src/transaction/check.rs`: `check::shielded_only`
- `zebra-consensus/src/transaction.rs`: rule wired into checks shared by block and mempool verification
- `zebra-consensus/src/error.rs`: four rejection errors, each scored as peer misbehavior
- `zebra-network/src/config.rs`: `shielded_only_height` in `zebrad.toml`

**Mining.** On a live node, blocks 1 to 4 paid their full 6.25-coin rewards into the
Ironwood shielded pool, leaving every other pool at zero. A block paying a transparent
address was rejected with `ShieldedOnlyTransparentOutput` and never joined the chain.
Zebra now also refuses to build templates on a shielded-only network unless
`miner_address` is a unified address with an Orchard receiver, logging why at startup.

`hybrid/mine-block.py` mines blocks while `disable_pow = true`: it fetches a template,
fills in a zero nonce and solution, and submits the block.

## Tests

Run each with `cargo nextest run`:

| Crate | Test | Checks |
| --- | --- | --- |
| zebra-chain | `shielded_only_heights_and_coinbase_payment_rules` | Accessors, startup guard, Regtest support |
| zebra-network | `shielded_only_height_serialization_roundtrip` | TOML round trip |
| zebra-network | `hybrid_testnet_config_parses` | Shipped config builds a shielded-only network |
| zebra-consensus | `shielded_only_activation_boundary` | Height gating; off on Mainnet and public Testnet |
| zebra-consensus | `shielded_only_classifies_historical_transactions` | Every Mainnet and Testnet vector transaction classified correctly |
| zebra-consensus | `shielded_only_rejects_transparent_coinbase_in_blocks` | Rule fires through the block verifier |
| zebra-chain | `custom_genesis_*` (3 tests) | Genesis builder output, hash inputs, message limits |
| zebra-rpc | `shielded_only_networks_require_orchard_miner_address` | Miner address guard |

```bash
cargo nextest run -p zebra-chain -p zebra-network -p zebra-consensus --lib -E 'test(/shielded_only|hybrid_testnet/)'
```

## Running the network

```bash
cargo build --release --bin zebrad
# Edit miner_address in hybrid/hybrid-testnet.toml to a unified address with an Orchard receiver.
./target/release/zebrad -c hybrid/hybrid-testnet.toml start

# First node only: load the genesis block. Other nodes download it from peers.
curl -s -H 'Content-Type: application/json' http://127.0.0.1:18232 \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"submitblock\",\"params\":[\"$(cat hybrid/genesis-block.hex)\"]}"
```

Mine blocks (proof of work disabled):

```bash
python3 hybrid/mine-block.py --count 10
```

To mint a different genesis block (new message, time, or difficulty):

```bash
cargo run -p zebra-chain --example hybrid-genesis -- \
  --message "your message, 75 bytes max" --time <unix seconds in the past> \
  --out hybrid/genesis-block.hex
```

Then copy the printed hash into `genesis_hash` and wipe each node's state directory.

Proof of work is disabled in this config until RandomX lands.

## Next steps, in order

1. **Spend a shielded coin.** Mined coins need a wallet that holds the miner address's
   spending key to move them. Set up a wallet (see step 6) and send a shielded payment
   between two addresses on this network.
2. **NU7 economics.** Zcash NU7 already implements a fee burn (ZIP 235) and reissuance
   of burned value (ZIP 234/237). Test activating `"NU7" = 1` and tune
   `initial_nsm_value_balance` and halving intervals. This gives the capped emission plus
   sustainable issuance from the design without new consensus code.
3. **RandomX proof of work.** Replace Equihash checks in
   `zebra-consensus/src/block/check.rs` (`equihash_solution_is_valid`,
   `difficulty_is_valid`) and the header solution type in `zebra-chain/src/work/`.
   This is a hard fork of the header format and the largest code change.
4. **Dandelion++.** Change transaction relay in `zebrad/src/components/mempool/gossip.rs`
   to send new transactions along a random stem before broadcasting. Add Tor/I2P docs.
5. **Address prefixes and branding.** Custom networks currently reuse Zcash Testnet
   address prefixes. Give the chain its own HRPs and coin type.
6. **Wallet.** Fork Zallet or a librustzcash-based light wallet plus Zaino for light
   clients. Viewing key export comes from librustzcash.
7. **Audit and public testnet** before any mainnet.

## Known gaps

- Mempool: when the rule activates at a height above 1, transactions accepted just
  before activation are not purged until they fail block validation. The Orchard
  soft fork handles this in `zebra-state/src/service/chain_tip.rs`; copy that if the
  chain ever activates the rule mid-life. Activating at height 1 avoids it.
- No shielded payment has been made yet, only coinbase rewards.
- The test blocks paid Zebra's public test address, which nobody here holds the key for.
- Upstream Zebra's contribution rules apply to PRs to Zebra itself. This branch is a
  separate project and is not meant to be sent upstream.
