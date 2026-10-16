#!/usr/bin/env python3
"""Mine one block on a hybrid network that runs with disable_pow = true.

Asks the node for a block template, assembles the block with an all-zero nonce and
Equihash solution, and submits it. Only for networks with proof of work disabled.

Usage:
    python3 hybrid/mine-block.py [--rpc http://127.0.0.1:18232] [--count N]

The node must have `miner_address` set to a shielded (Orchard) unified address and,
on a network with no peers, `[mempool] debug_enable_at_height = 0`.
"""

import argparse
import json
import sys
import urllib.request

SOLUTION_SIZE = 1344


def rpc(url, method, params=None):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or []})
    req = urllib.request.Request(url, body.encode(), {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as resp:
        reply = json.load(resp)
    if reply.get("error"):
        raise RuntimeError(f"{method} failed: {reply['error']}")
    return reply["result"]


def compact_size(n):
    if n < 0xFD:
        return bytes([n])
    if n <= 0xFFFF:
        return b"\xfd" + n.to_bytes(2, "little")
    return b"\xfe" + n.to_bytes(4, "little")


def block_from_template(t):
    """Serializes a block from a getblocktemplate result."""
    # Hashes in RPC results are in display order: reverse them for serialization.
    header = (
        t["version"].to_bytes(4, "little")
        + bytes.fromhex(t["previousblockhash"])[::-1]
        + bytes.fromhex(t["defaultroots"]["merkleroot"])[::-1]
        + bytes.fromhex(t["defaultroots"]["blockcommitmentshash"])[::-1]
        + t["curtime"].to_bytes(4, "little")
        + bytes.fromhex(t["bits"])[::-1]
        + bytes(32)  # nonce
        + compact_size(SOLUTION_SIZE)
        + bytes(SOLUTION_SIZE)  # Equihash solution, unchecked with disable_pow
    )
    txs = [t["coinbasetxn"]["data"]] + [tx["data"] for tx in t["transactions"]]
    return header + compact_size(len(txs)) + b"".join(bytes.fromhex(tx) for tx in txs)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--rpc", default="http://127.0.0.1:18232")
    parser.add_argument("--count", type=int, default=1, help="blocks to mine")
    args = parser.parse_args()

    for _ in range(args.count):
        template = rpc(args.rpc, "getblocktemplate")
        block = block_from_template(template)
        result = rpc(args.rpc, "submitblock", [block.hex()])
        if result is not None:
            sys.exit(f"block {template['height']} rejected: {result}")
        best = rpc(args.rpc, "getbestblockhash")
        print(f"mined block {template['height']}: {best}")


if __name__ == "__main__":
    main()
