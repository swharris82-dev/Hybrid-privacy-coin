//! Mints the genesis block for a custom network, such as the hybrid privacy chain.
//!
//! Usage:
//!
//! ```sh
//! cargo run -p zebra-chain --example hybrid-genesis -- \
//!     --message "HybridTestnet genesis 2026-10-14" \
//!     --time 1792000000 \
//!     --bits 200f0f0f \
//!     --out hybrid/genesis-block.hex
//! ```
//!
//! Prints the genesis block hash, which goes in `genesis_hash` in the network config, and
//! writes the serialized block as hex, which is loaded into the first node with the
//! `submitblock` RPC.
//!
//! The block has an all-zero nonce and Equihash solution, so it is only valid on networks
//! with `disable_pow = true`. Re-mint it once the chain's proof of work is final.

// This is a command-line tool, so printing is its output.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::{env, fs, process::ExitCode};

use zebra_chain::{
    block::genesis::{custom_genesis_block, CustomGenesisParameters, GENESIS_SOLUTION_SIZE},
    serialization::ZcashSerialize,
    work::difficulty::CompactDifficulty,
};

const USAGE: &str = "usage: hybrid-genesis --message <text> --time <unix seconds> \
                     [--bits <compact hex, default 200f0f0f>] [--out <file>]";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut message = None;
    let mut time = None;
    let mut bits = 0x200f_0f0f_u32;
    let mut out = None;

    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--message" => message = Some(value()?),
            "--time" => {
                time = Some(
                    value()?
                        .parse::<u32>()
                        .map_err(|e| format!("--time must be unix seconds that fit in u32: {e}"))?,
                )
            }
            "--bits" => {
                let raw = value()?;
                bits = u32::from_str_radix(raw.trim_start_matches("0x"), 16)
                    .map_err(|e| format!("--bits must be hex: {e}"))?
            }
            "--out" => out = Some(value()?),
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }

    let message = message.ok_or("--message is required")?;
    let time = time.ok_or("--time is required")?;

    let difficulty_threshold = CompactDifficulty::from_bytes_in_display_order(&bits.to_be_bytes())
        .map_err(|e| format!("--bits is not a valid compact difficulty: {e}"))?;

    let block = custom_genesis_block(&CustomGenesisParameters {
        time,
        difficulty_threshold,
        nonce: [0; 32],
        solution: [0; GENESIS_SOLUTION_SIZE],
        message: message.into_bytes(),
    })
    .map_err(|e| e.to_string())?;

    let block_hex = hex::encode(
        block
            .zcash_serialize_to_vec()
            .map_err(|e| format!("serializing genesis block: {e}"))?,
    );

    if let Some(path) = &out {
        fs::write(path, format!("{block_hex}\n")).map_err(|e| format!("writing {path}: {e}"))?;
        eprintln!("wrote genesis block to {path}");
    } else {
        println!("{block_hex}");
    }

    eprintln!("genesis hash: {}", block.hash());
    eprintln!(
        "add to [network.testnet_parameters]: genesis_hash = \"{}\"",
        block.hash()
    );

    Ok(())
}
