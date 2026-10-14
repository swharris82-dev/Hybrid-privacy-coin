//! Genesis blocks: the hard-coded Regtest genesis block, and a builder for the genesis blocks
//! of custom networks.

use std::{io, sync::Arc};

use hex::FromHex;
use thiserror::Error;

use crate::{
    block::{merkle, Block, Header},
    parameters::GENESIS_PREVIOUS_BLOCK_HASH,
    serialization::{SerializationError, ZcashDeserializeInto, ZcashSerialize},
    transaction::Transaction,
    transparent::serialize::GENESIS_COINBASE_SCRIPT_SIG,
    work::{difficulty::CompactDifficulty, equihash::SOLUTION_SIZE},
};

/// Genesis block for Regtest, copied from zcashd via `getblock 0 0` RPC method
pub fn regtest_genesis_block() -> Arc<Block> {
    let regtest_genesis_block_bytes =
        <Vec<u8>>::from_hex(include_str!("genesis/block-regtest-0-000-000.txt").trim())
            .expect("Block bytes are in valid hex representation");

    regtest_genesis_block_bytes
        .zcash_deserialize_into()
        .map(Arc::new)
        .expect("hard-coded Regtest genesis block data must deserialize successfully")
}

/// The maximum length of a custom genesis block message, in bytes.
///
/// The message is stored in a single `OP_RETURN` push. 75 bytes is the largest push that
/// fits in a one-byte push opcode, which keeps the output script standard.
pub const MAX_GENESIS_MESSAGE_LEN: usize = 75;

/// The size of the Equihash solution in a Mainnet or Testnet-style block header, in bytes.
pub const GENESIS_SOLUTION_SIZE: usize = SOLUTION_SIZE;

/// The `OP_RETURN` script opcode, which makes an output provably unspendable.
const OP_RETURN: u8 = 0x6a;

/// The block header version used by all Zcash blocks.
const GENESIS_BLOCK_VERSION: u32 = 4;

/// Parameters for building the genesis block of a custom network.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomGenesisParameters {
    /// The block time, as seconds since the Unix epoch.
    pub time: u32,

    /// The compact difficulty threshold (`nBits`) in the header.
    ///
    /// This must not be above the network's target difficulty limit, or the node rejects
    /// the block.
    pub difficulty_threshold: CompactDifficulty,

    /// The 32-byte header nonce.
    pub nonce: [u8; 32],

    /// The 1344-byte Equihash solution.
    ///
    /// Networks with `disable_pow = true` accept any solution. Networks that check proof of
    /// work need a solution that is valid for the rest of the header.
    pub solution: [u8; SOLUTION_SIZE],

    /// A message embedded in the genesis coinbase transaction.
    ///
    /// It is stored in a zero-value `OP_RETURN` output, so it can never be spent.
    pub message: Vec<u8>,
}

/// An error building a custom genesis block.
#[derive(Debug, Error)]
pub enum CustomGenesisError {
    /// The message is longer than [`MAX_GENESIS_MESSAGE_LEN`].
    #[error("genesis message is {len} bytes, but must be at most {MAX_GENESIS_MESSAGE_LEN} bytes")]
    MessageTooLong {
        /// The length of the rejected message.
        len: usize,
    },

    /// The constructed transaction or block failed to serialize or deserialize.
    #[error("constructed genesis data is invalid: {0}")]
    Serialization(#[from] SerializationError),

    /// Writing the block bytes failed.
    #[error("writing genesis block bytes failed: {0}")]
    Io(#[from] io::Error),
}

/// Builds the genesis block for a custom network.
///
/// The block has the same structure as the Zcash genesis blocks: a version 1 coinbase
/// transaction with Zcash's genesis coinbase script, which Zebra and zcashd recognize as
/// height 0. The custom `message` goes in the coinbase's only output, which pays zero value
/// to an `OP_RETURN` script.
///
/// The network's `genesis_hash` must be set to the returned block's hash.
pub fn custom_genesis_block(
    parameters: &CustomGenesisParameters,
) -> Result<Arc<Block>, CustomGenesisError> {
    let CustomGenesisParameters {
        time,
        difficulty_threshold,
        nonce,
        solution,
        message,
    } = parameters;

    if message.len() > MAX_GENESIS_MESSAGE_LEN {
        return Err(CustomGenesisError::MessageTooLong { len: message.len() });
    }

    let coinbase = Arc::new(genesis_coinbase_transaction(message)?);
    let merkle_root: merkle::Root = std::iter::once(coinbase.clone()).collect();

    let mut block_bytes = Vec::new();

    // Header
    block_bytes.extend_from_slice(&GENESIS_BLOCK_VERSION.to_le_bytes());
    block_bytes.extend_from_slice(&GENESIS_PREVIOUS_BLOCK_HASH.0);
    block_bytes.extend_from_slice(&merkle_root.0);
    // Zcash genesis blocks commit to an all-zero value.
    block_bytes.extend_from_slice(&[0; 32]);
    block_bytes.extend_from_slice(&time.to_le_bytes());
    block_bytes.extend_from_slice(&difficulty_threshold.0.to_le_bytes());
    block_bytes.extend_from_slice(nonce);
    write_compact_size(&mut block_bytes, SOLUTION_SIZE);
    block_bytes.extend_from_slice(solution);

    // Transactions
    write_compact_size(&mut block_bytes, 1);
    coinbase.zcash_serialize(&mut block_bytes)?;

    let block: Block = block_bytes.zcash_deserialize_into()?;
    Ok(Arc::new(block))
}

/// Returns the header of a custom genesis block, for proof of work searches that only vary
/// the header fields.
pub fn custom_genesis_header(
    parameters: &CustomGenesisParameters,
) -> Result<Arc<Header>, CustomGenesisError> {
    Ok(custom_genesis_block(parameters)?.header.clone())
}

/// Builds the version 1 genesis coinbase transaction carrying `message`.
fn genesis_coinbase_transaction(message: &[u8]) -> Result<Transaction, CustomGenesisError> {
    let mut tx = Vec::new();

    // version 1
    tx.extend_from_slice(&1u32.to_le_bytes());

    // one coinbase input: null prevout, Zcash's genesis coinbase script, final sequence
    write_compact_size(&mut tx, 1);
    tx.extend_from_slice(&[0; 32]);
    tx.extend_from_slice(&u32::MAX.to_le_bytes());
    write_compact_size(&mut tx, GENESIS_COINBASE_SCRIPT_SIG.len());
    tx.extend_from_slice(&GENESIS_COINBASE_SCRIPT_SIG);
    tx.extend_from_slice(&u32::MAX.to_le_bytes());

    // one zero-value output: OP_RETURN <message>
    write_compact_size(&mut tx, 1);
    tx.extend_from_slice(&0u64.to_le_bytes());
    let mut script = vec![OP_RETURN];
    if !message.is_empty() {
        // Checked against `MAX_GENESIS_MESSAGE_LEN` (75) by the caller, so this is a
        // single-byte push opcode and the cast cannot truncate.
        script.push(message.len() as u8);
        script.extend_from_slice(message);
    }
    write_compact_size(&mut tx, script.len());
    tx.extend_from_slice(&script);

    // lock time
    tx.extend_from_slice(&0u32.to_le_bytes());

    Ok(tx.zcash_deserialize_into()?)
}

/// Writes a Bitcoin-style CompactSize length prefix.
fn write_compact_size(bytes: &mut Vec<u8>, len: usize) {
    // Lengths here are at most `SOLUTION_SIZE` (1344), so they fit in a u16. The casts are
    // guarded by the match arms.
    match len {
        0..=0xfc => bytes.push(len as u8),
        0xfd..=0xffff => {
            bytes.push(0xfd);
            bytes.extend_from_slice(&(len as u16).to_le_bytes());
        }
        _ => unreachable!("genesis block lengths are at most SOLUTION_SIZE"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        block::Height,
        serialization::ZcashSerialize,
        transparent::{Input, Script},
    };

    fn parameters(message: &str) -> CustomGenesisParameters {
        CustomGenesisParameters {
            time: 1_792_000_000,
            difficulty_threshold: CompactDifficulty(0x200f_0f0f),
            nonce: [0; 32],
            solution: [0; SOLUTION_SIZE],
            message: message.as_bytes().to_vec(),
        }
    }

    #[test]
    fn custom_genesis_block_is_a_valid_genesis_block() {
        let _init_guard = zebra_test::init();

        let block = custom_genesis_block(&parameters("HybridTestnet genesis")).unwrap();

        assert_eq!(block.coinbase_height(), Some(Height(0)));
        assert_eq!(
            block.header.previous_block_hash,
            GENESIS_PREVIOUS_BLOCK_HASH
        );
        assert_eq!(block.transactions.len(), 1);
        assert!(block.transactions[0].is_coinbase());

        // The merkle root commits to the coinbase transaction.
        let expected_root: merkle::Root = block.transactions.iter().collect();
        assert_eq!(block.header.merkle_root, expected_root);

        // The message is in an unspendable zero-value output.
        let outputs = block.transactions[0].outputs();
        assert_eq!(outputs.len(), 1);
        assert_eq!(u64::from(outputs[0].value), 0);
        let mut expected_script = vec![OP_RETURN, 21];
        expected_script.extend_from_slice(b"HybridTestnet genesis");
        assert_eq!(outputs[0].lock_script, Script::new(&expected_script));

        // The coinbase input is Zcash's genesis input.
        assert!(matches!(
            block.transactions[0].inputs()[0],
            Input::Coinbase {
                height: Height(0),
                ..
            }
        ));

        // The block round-trips through serialization unchanged.
        let bytes = block.zcash_serialize_to_vec().unwrap();
        let reparsed: Block = bytes.zcash_deserialize_into().unwrap();
        assert_eq!(reparsed.hash(), block.hash());
    }

    #[test]
    fn custom_genesis_hash_depends_on_every_parameter() {
        let _init_guard = zebra_test::init();

        let base = parameters("a");
        let base_hash = custom_genesis_block(&base).unwrap().hash();

        let mut changed = base.clone();
        changed.message = b"b".to_vec();
        assert_ne!(custom_genesis_block(&changed).unwrap().hash(), base_hash);

        let mut changed = base.clone();
        changed.time += 1;
        assert_ne!(custom_genesis_block(&changed).unwrap().hash(), base_hash);

        let mut changed = base.clone();
        changed.nonce[0] = 1;
        assert_ne!(custom_genesis_block(&changed).unwrap().hash(), base_hash);

        let mut changed = base;
        changed.difficulty_threshold = CompactDifficulty(0x200f_0f0e);
        assert_ne!(custom_genesis_block(&changed).unwrap().hash(), base_hash);
    }

    #[test]
    fn custom_genesis_message_limits() {
        let _init_guard = zebra_test::init();

        let longest = "x".repeat(MAX_GENESIS_MESSAGE_LEN);
        assert!(custom_genesis_block(&parameters(&longest)).is_ok());
        assert!(custom_genesis_block(&parameters("")).is_ok());

        let too_long = "x".repeat(MAX_GENESIS_MESSAGE_LEN + 1);
        assert!(matches!(
            custom_genesis_block(&parameters(&too_long)),
            Err(CustomGenesisError::MessageTooLong { len }) if len == MAX_GENESIS_MESSAGE_LEN + 1
        ));
    }
}
