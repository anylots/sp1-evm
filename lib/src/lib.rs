use std::{collections::HashMap, str::FromStr};

use alloy_sol_types::sol;
use revm::{
    primitives::{b256, Address, TxKind, B256, U256},
    Evm,
};

sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    struct PublicValuesStruct {
        uint32 n;
        uint32 a;
        uint32 b;
    }
}

/// EVM executor that handles the block.
// pub struct EvmExecutor {
//     hardfork_config: HardforkConfig,
//     db: CacheDB<ReadOnlyDB>,
//     zktrie_db: Rc<ZkMemoryDb>,
//     zktrie: ZkTrie<UpdateDb>,
//     spec_id: SpecId,
//     hooks: hooks::ExecuteHooks,
// }

pub const KECCAK_EMPTY: B256 =
    b256!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");

pub fn exec(n: u32) -> (u32, u32) {
    for _ in 0..20 {
        let mut cache_state = revm::CacheState::new(false);

        let acc_info = revm::primitives::AccountInfo {
            balance: U256::from(10u64.pow(18)),
            #[cfg(feature = "scroll")]
            code_size,
            code_hash: KECCAK_EMPTY,
            #[cfg(feature = "scroll-poseidon-codehash")]
            poseidon_code_hash,
            code: None,
            nonce: 0,
        };
        cache_state.insert_account_with_storage(
            Address::from_str("0x0000000000000000000000000000000000000001").unwrap(),
            acc_info,
            HashMap::new(),
        );

        let state = revm::db::State::builder()
            .with_cached_prestate(cache_state)
            .with_bundle_update()
            .build();

        // let mut env = Box::<Env>::default();
        // env.cfg.chain_id = 1;
        // env.tx = TxEnv::default();

        // let mut evm = Evm::builder()
        //     .with_db(&mut state)
        //     .modify_env(|e| e.clone_from(&env))
        //     .with_spec_id(SpecId::MERGE)
        //     .build();
        // let exec_result = evm.transact_commit();
        // println!("\nExecution result: {exec_result:#?}");

        let mut evm = Evm::builder()
            .with_db(state)
            .modify_tx_env(|tx| {
                // execution globals block hash/gas_limit/coinbase/timestamp..
                tx.caller = "0x0000000000000000000000000000000000000001"
                    .parse()
                    .unwrap();
                tx.value = U256::from(10);
                tx.transact_to = TxKind::Call(
                    "0x0000000000000000000000000000000000000000"
                        .parse()
                        .unwrap(),
                );
            })
            .build();
        let exec_result = evm.transact();
        assert!(
            exec_result.is_ok(),
            "{}",
            format!("{}", exec_result.unwrap_err())
        );
        println!("\nExecution result: {exec_result:#?}");
    }

    (n + 1, 2)
}
