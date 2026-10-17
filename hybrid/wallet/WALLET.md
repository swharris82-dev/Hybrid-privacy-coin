# Wallet

The Lacuna network uses Zcash's own wallet stack, with one small patch:

```text
zebrad (this repo)  --JSON-RPC-->  zainod (Zaino)  --gRPC-->  zcash-devtool (patched)
```

- **Zaino** serves light wallets. Run it in `Regtest` mode: it reads the activation
  heights from the node at startup, so it needs no changes.
- **zcash-devtool** is Zcash's command-line wallet and already supports Ironwood.
  `zcash-devtool-custom-test.patch` adds `-n custom-test`: Testnet-style addresses
  (`utest1…`) with this network's activation heights.

Tested end to end on 2026-10-17: 110 mined blocks paid wallet A, wallet A sent 100 coins
to wallet B with a memo, and wallet B received them, read the memo, and could spend them
after 10 confirmations. On chain the payment showed only 17 Ironwood actions and the
0.00085 fee: no addresses, amounts, or memo.

## Versions tested

| Component | Source | Commit |
| --- | --- | --- |
| zcash-devtool | https://github.com/zcash/zcash-devtool | `0b3436b` + patch |
| Zaino | https://github.com/zingolabs/zaino | `25c1f2e` |

## Setup

```bash
# 1. Build Zaino
git clone https://github.com/zingolabs/zaino && cd zaino
cargo build --release -p zainod

# 2. Build the patched wallet
git clone https://github.com/zcash/zcash-devtool && cd zcash-devtool
git checkout 0b3436b
git am /path/to/hybrid/wallet/zcash-devtool-custom-test.patch
cargo build --release --features regtest_support
```

## Running

1. Start the node and load the genesis block (see `HYBRID.md`). Mine at least one block:
   the wallet needs a block after genesis to set its starting point.
2. Start Zaino. Edit the two paths in `zainod.toml` first.

   ```bash
   zainod start -c hybrid/wallet/zainod.toml
   ```

3. Create a wallet:

   ```bash
   zcash-devtool wallet -w my-wallet init --name "My wallet" -i my-wallet.age \
     -n custom-test --activation-heights hybrid/wallet/activation-heights.toml \
     -s 127.0.0.1:8137
   zcash-devtool wallet -w my-wallet list-addresses
   ```

   `my-wallet.age` holds the key that unlocks the wallet's seed phrase. Back up both,
   and also write down the seed (`wallet display-mnemonic`). Lose them and the coins are gone.

4. To mine to this wallet, put its address in `miner_address` in the node config and
   restart the node.
5. Everyday use:

   ```bash
   zcash-devtool wallet -w my-wallet sync -s 127.0.0.1:8137
   zcash-devtool wallet -w my-wallet balance
   zcash-devtool wallet -w my-wallet send -i my-wallet.age \
     --address <utest1…> --value <zatoshis> --memo "text" -s 127.0.0.1:8137
   zcash-devtool wallet -w my-wallet enhance -s 127.0.0.1:8137   # decrypts memos
   zcash-devtool wallet -w my-wallet list-tx
   ```

## Known gaps

- The wallet labels amounts "TAZ" (Zcash testnet coins). Branding comes with the
  network's own address prefixes.
- Mined coins become spendable after 100 confirmations; received coins after 10.
- Command line only. A phone or desktop wallet would be a fork of Zashi or Zingo.
