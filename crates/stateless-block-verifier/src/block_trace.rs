use std::collections::HashMap;

use ethers_core::types::{
    transaction::{
        eip2930::{AccessList, AccessListItem},
        response::Transaction,
    },
    Address, Block, Bytes, H256, U256, U64,
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Map};

/// Bytecode
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Deserialize,
    Serialize,
    Default,
    Debug,
    Clone,
    Eq,
    PartialEq,
)]
#[archive(check_bytes)]
#[archive_attr(derive(Debug, Hash, PartialEq, Eq))]
pub struct BytecodeTrace {
    /// poseidon code hash
    pub hash: H256,
    /// bytecode
    pub code: Bytes,
}

/// l2 block full trace
#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct BlockTrace {
    /// Version string
    //pub version: String,
    /// chain id
    #[serde(rename = "chainID", default)]
    pub chain_id: u64,
    /// coinbase's status AFTER execution
    pub coinbase: AccountTrace,
    /// block
    pub header: EthBlock,
    /// txs
    pub transactions: Vec<TransactionTrace>,
    /// execution results
    #[serde(rename = "executionResults", default)]
    pub execution_results: Vec<ExecutionResult>,
    /// Accessed bytecodes with hashes
    #[serde(default)]
    pub codes: Vec<BytecodeTrace>,
    /// storage trace BEFORE execution
    #[serde(rename = "storageTrace")]
    pub storage_trace: StorageTrace,
    /// per-tx storage used by ccc
    #[serde(rename = "txStorageTraces", default)]
    pub tx_storage_trace: Vec<StorageTrace>,
    /// l1 tx queue
    #[serde(rename = "startL1QueueIndex", default)]
    pub start_l1_queue_index: u64,
    /// Withdraw root
    pub withdraw_trie_root: H256,
}

/// Main Block type
pub type EthBlock = Block<Transaction>;
/// Ethereum Hash (256 bits).
pub type Hash = H256;

impl From<BlockTrace> for EthBlock {
    fn from(b: BlockTrace) -> Self {
        let mut txs = Vec::new();
        for (idx, tx_data) in b.transactions.iter().enumerate() {
            let tx_idx = Some(U64::from(idx));
            let tx = tx_data.to_eth_tx(
                b.header.hash,
                b.header.number,
                tx_idx,
                b.header.base_fee_per_gas,
            );
            txs.push(tx)
        }
        EthBlock { transactions: txs, difficulty: 0.into(), ..b.header }
    }
}

impl From<&BlockTrace> for EthBlock {
    fn from(b: &BlockTrace) -> Self {
        let mut txs = Vec::new();
        for (idx, tx_data) in b.transactions.iter().enumerate() {
            let tx_idx = Some(U64::from(idx));
            let tx = tx_data.to_eth_tx(
                b.header.hash,
                b.header.number,
                tx_idx,
                b.header.base_fee_per_gas,
            );
            txs.push(tx)
        }
        EthBlock { transactions: txs, difficulty: 0.into(), ..b.header.clone() }
    }
}

/// l2 tx trace
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Deserialize,
    Serialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[archive(check_bytes)]
#[archive_attr(derive(Debug, Hash, PartialEq, Eq))]
pub struct TransactionTrace {
    // FIXME after traces upgraded
    /// tx hash
    #[serde(default, rename = "txHash")]
    pub tx_hash: H256,
    /// tx type (in raw from)
    #[serde(rename = "type")]
    pub type_: u8,
    /// nonce
    pub nonce: u64,
    /// gas limit
    pub gas: u64,
    #[serde(rename = "gasPrice")]
    /// gas price
    pub gas_price: U256,
    #[serde(rename = "gasTipCap")]
    /// gas tip cap
    pub gas_tip_cap: Option<U256>,
    #[serde(rename = "gasFeeCap")]
    /// gas fee cap
    pub gas_fee_cap: Option<U256>,
    /// from
    pub from: Address,
    /// to, NONE for creation (0 addr)
    pub to: Option<Address>,
    /// chain id
    #[serde(rename = "chainId")]
    pub chain_id: U256,
    /// value amount
    pub value: U256,
    /// call data
    pub data: Bytes,
    /// is creation
    #[serde(rename = "isCreate")]
    pub is_create: bool,
    /// access list
    #[serde(rename = "accessList")]
    pub access_list: Option<Vec<AccessListItem>>,
    /// signature v
    pub v: U64,
    /// signature r
    pub r: U256,
    /// signature s
    pub s: U256,
}

impl TransactionTrace {
    /// Check whether it is layer1 tx
    pub fn is_l1_tx(&self) -> bool {
        self.type_ == 0x7e
    }

    /// transfer to eth type tx
    pub fn to_eth_tx(
        &self,
        block_hash: Option<H256>,
        block_number: Option<U64>,
        transaction_index: Option<U64>,
        base_fee_per_gas: Option<U256>,
    ) -> Transaction {
        let gas_price = if self.type_ == 2 {
            let priority_fee_per_gas = std::cmp::min(
                self.gas_tip_cap.unwrap(),
                self.gas_fee_cap.unwrap() - base_fee_per_gas.unwrap(),
            );
            let effective_gas_price = priority_fee_per_gas + base_fee_per_gas.unwrap();
            effective_gas_price
        } else {
            self.gas_price
        };
        Transaction {
            hash: self.tx_hash,
            nonce: U256::from(self.nonce),
            block_hash,
            block_number,
            transaction_index,
            from: self.from,
            to: self.to,
            value: self.value,
            gas_price: Some(gas_price),
            gas: U256::from(self.gas),
            input: self.data.clone(),
            v: self.v,
            r: self.r,
            s: self.s,
            // FIXME: is this correct? None for legacy?
            transaction_type: Some(U64::from(self.type_ as u64)),
            access_list: self.access_list.as_ref().map(|al| AccessList(al.clone())),
            max_priority_fee_per_gas: self.gas_tip_cap,
            max_fee_per_gas: self.gas_fee_cap,
            chain_id: if self.type_ != 0 || self.v.as_u64() >= 35 {
                Some(self.chain_id)
            } else {
                None
            },
            other: Default::default(),
        }
    }
}

/// account trie proof in storage proof
pub type AccountTrieProofs = Vec<(Address, Vec<Bytes>)>;
/// storage trie proof in storage proof
pub type StorageTrieProofs = Vec<(Address, Vec<(H256, Vec<Bytes>)>)>;

/// storage trace
#[serde_as]
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Deserialize,
    Serialize,
    Default,
    Debug,
    Clone,
    Eq,
    PartialEq,
)]
#[archive(check_bytes)]
#[archive_attr(derive(Debug, PartialEq, Eq))]
pub struct StorageTrace {
    /// root before
    #[serde(rename = "rootBefore")]
    pub root_before: Hash,
    /// root after
    #[serde(rename = "rootAfter")]
    pub root_after: Hash,
    /// account proofs
    #[serde(default)]
    #[serde_as(as = "Map<_, _>")]
    pub proofs: AccountTrieProofs,
    #[serde(rename = "storageProofs", default)]
    #[serde_as(as = "Map<_, Map<_, _>>")]
    /// storage proofs for each account
    pub storage_proofs: StorageTrieProofs,
    #[serde(rename = "deletionProofs", default)]
    /// additional deletion proofs
    pub deletion_proofs: Vec<Bytes>,
    #[serde(rename = "flattenProofs", default)]
    #[serde_as(as = "Map<_, _>")]
    ///
    pub flatten_proofs: Vec<(H256, Bytes)>,
    #[serde(rename = "addressHashes", default)]
    #[serde_as(as = "Map<_, _>")]
    ///
    pub address_hashes: Vec<(Address, Hash)>,
    #[serde(rename = "storeKeyHashes", default)]
    #[serde_as(as = "Map<_, _>")]
    ///
    pub store_key_hashes: Vec<(H256, Hash)>,
}

/// extension of `GethExecTrace`, with compatible serialize form
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ExecutionResult {
    /// L1 fee
    #[serde(rename = "l1DataFee", default)]
    pub l1_fee: U256,
    /// used gas
    pub gas: u64,
    /// True when the transaction has failed.
    pub failed: bool,
    /// Return value of execution which is a hex encoded byte array
    #[serde(rename = "returnValue", default)]
    pub return_value: String,
    /// Status of from account AFTER execution
    /// TODO: delete this
    pub from: Option<AccountTrace>,
    /// Status of to account AFTER execution
    /// TODO: delete this after curie upgrade
    pub to: Option<AccountTrace>,
    #[serde(rename = "accountAfter", default)]
    /// List of accounts' (coinbase etc) status AFTER execution
    pub account_after: Vec<AccountTrace>,
    #[serde(rename = "accountCreated")]
    /// Status of created account AFTER execution
    /// TODO: delete this
    pub account_created: Option<AccountTrace>,
    #[serde(rename = "poseidonCodeHash")]
    /// code hash of called
    pub code_hash: Option<Hash>,
    #[serde(rename = "byteCode")]
    /// called code
    pub byte_code: Option<String>,
    // #[serde(rename = "structLogs")]
    /// Exec steps
    // pub exec_steps: Vec<ExecStep>,
    /// callTrace
    #[serde(rename = "callTrace")]
    pub call_trace: GethCallTrace,
    /// prestate
    #[serde(default)]
    pub prestate: HashMap<Address, GethPrestateTrace>,
}

/// The call trace returned by geth RPC debug_trace* methods.
/// using callTracer
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq)]
pub struct GethCallTrace {
    #[serde(default)]
    calls: Vec<GethCallTrace>,
    error: Option<String>,
    from: Address,
    // gas: U256,
    #[serde(rename = "gasUsed")]
    gas_used: U256,
    // input: Bytes,
    output: Option<Bytes>,
    to: Option<Address>,
    #[serde(rename = "type")]
    call_type: String,
    // value: U256,
}

/// The prestate trace returned by geth RPC debug_trace* methods.
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GethPrestateTrace {
    /// balance
    pub balance: Option<U256>,
    /// nonce
    pub nonce: Option<u64>,
    /// code
    pub code: Option<Bytes>,
    /// storage
    pub storage: Option<HashMap<U256, U256>>,
}

/// account wrapper for account status
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Serialize,
    Deserialize,
    Clone,
    Default,
    Debug,
    PartialEq,
    Eq,
)]
#[archive(check_bytes)]
#[archive_attr(derive(Debug, Hash, PartialEq, Eq))]
#[doc(hidden)]
pub struct AccountTrace {
    pub address: Address,
    pub nonce: u64,
    pub balance: U256,
    #[serde(rename = "keccakCodeHash")]
    pub keccak_code_hash: H256,
    #[serde(rename = "poseidonCodeHash")]
    pub poseidon_code_hash: H256,
    #[serde(rename = "codeSize")]
    pub code_size: u64,
}

/// Tx type
#[derive(Default, Debug, Copy, Clone, Serialize, PartialEq, Eq)]
pub enum TxType {
    /// EIP 155 tx
    #[default]
    Eip155 = 0,
    /// Pre EIP 155 tx
    PreEip155,
    /// EIP 1559 tx
    Eip1559,
    /// EIP 2930 tx
    Eip2930,
    /// L1 Message tx
    L1Msg,
}

impl From<TxType> for usize {
    fn from(value: TxType) -> Self {
        value as usize
    }
}

impl From<TxType> for u64 {
    fn from(value: TxType) -> Self {
        value as u64
    }
}

impl TxType {
    /// If this type is L1Msg or not
    pub fn is_l1_msg(&self) -> bool {
        matches!(*self, Self::L1Msg)
    }

    /// If this type is PreEip155
    pub fn is_pre_eip155(&self) -> bool {
        matches!(*self, TxType::PreEip155)
    }

    /// If this type is EIP155 or not
    pub fn is_eip155(&self) -> bool {
        matches!(*self, TxType::Eip155)
    }

    /// If this type is Eip1559 or not
    pub fn is_eip1559(&self) -> bool {
        matches!(*self, TxType::Eip1559)
    }

    /// If this type is Eip2930 or not
    pub fn is_eip2930(&self) -> bool {
        matches!(*self, TxType::Eip2930)
    }

    /// Get the type of transaction
    pub fn get_tx_type(tx: &Transaction) -> Self {
        match tx.transaction_type {
            Some(x) if x == U64::from(1) => Self::Eip2930,
            Some(x) if x == U64::from(2) => Self::Eip1559,
            Some(x) if x == U64::from(0x7e) => Self::L1Msg,
            _ => {
                if cfg!(feature = "scroll") {
                    if tx.v.is_zero() && tx.r.is_zero() && tx.s.is_zero() {
                        Self::L1Msg
                    } else {
                        match tx.v.as_u64() {
                            0 | 1 | 27 | 28 => Self::PreEip155,
                            _ => Self::Eip155,
                        }
                    }
                } else {
                    match tx.v.as_u64() {
                        0 | 1 | 27 | 28 => Self::PreEip155,
                        _ => Self::Eip155,
                    }
                }
            }
        }
    }

    /// Return the recovery id of signature for recovering the signing pk
    pub fn get_recovery_id(&self, v: u64) -> u8 {
        let recovery_id = match *self {
            TxType::Eip155 => (v + 1) % 2,
            TxType::PreEip155 => {
                assert!(v == 0x1b || v == 0x1c, "v: {v}");
                v - 27
            }
            TxType::Eip1559 => {
                assert!(v <= 1);
                v
            }
            TxType::Eip2930 => {
                assert!(v <= 1);
                v
            }
            TxType::L1Msg => {
                unreachable!("L1 msg does not have signature")
            }
        };

        recovery_id as u8
    }
}
