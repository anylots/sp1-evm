//! Stateless Block Verifier

#![feature(lazy_cell)]
// #![deny(missing_docs)]
#![deny(missing_debug_implementations)]

#[cfg(feature = "dev")]
#[doc(hidden)]
pub use tracing;

#[macro_use]
mod macros;

mod chunk;

pub use chunk::ChunkInfo;

mod database;
pub use database::ReadOnlyDB;

mod error;
pub use error::VerificationError;

mod executor;
pub use executor::{hooks, EvmExecutor, EvmExecutorBuilder};

mod hardfork;
pub use hardfork::HardforkConfig;

/// Module for utilities.
pub mod utils;
use utils::ext::BlockZktrieExt;
pub use utils::{post_check, BlockTraceExt};

use eth_types::l2_types::BlockTrace;
use mpt_zktrie::ZktrieState;

/// Metrics module
#[cfg(feature = "metrics")]
#[doc(hidden)]
pub mod metrics;

#[cfg(all(feature = "dev", test))]
#[ctor::ctor]
fn init() {
    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}

use std::{collections::HashMap, str::FromStr};

use alloy_sol_types::sol;
use revm::{
    primitives::{b256, Address, TxKind, B256, U256},
    Evm,
};

sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    struct PublicValuesStruct {
        // n
        uint32 n;
        // a
        uint32 a;
        // b
        uint32 b;
    }
}

pub fn verify(l2_trace: &BlockTrace) -> Result<(), VerificationError> {
    dev_trace!("{l2_trace:#?}");
    let disable_checks = false;

    let mut fork_config = HardforkConfig::default_from_chain_id(534352);
    fork_config.set_curie_block(0);

    let root_after = l2_trace.storage_trace.root_after;
    println!("root_after: {:?}", root_after);

    // or with v2 trace
    // let v2_trace = BlockTraceV2::from(l2_trace.clone());

    // or with rkyv zero copy
    // let serialized = rkyv::to_bytes::<BlockTraceV2, 4096>(&v2_trace).unwrap();
    // let archived = unsafe { rkyv::archived_root::<BlockTraceV2>(&serialized[..]) };
    // let archived = rkyv::check_archived_root::<BlockTraceV2>(&serialized[..]).unwrap();

    #[cfg(feature = "profiling")]
    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(1000)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .unwrap();

    let mut zktrie_state = cycle_track!(
        {
            let old_root = l2_trace.storage_trace.root_before;
            let mut zktrie_state = ZktrieState::construct(old_root);
            l2_trace.build_zktrie_state(&mut zktrie_state);
            zktrie_state
        },
        "build ZktrieState"
    );

    let mut executor = EvmExecutorBuilder::new(&zktrie_state)
        .hardfork_config(fork_config)
        .with_execute_hooks(|hooks| {
            let l2_trace = l2_trace.clone();
            if !disable_checks {
                hooks.add_post_tx_execution_handler(move |executor, tx_id| {
                    post_check(executor.db(), &l2_trace.execution_results[tx_id]);
                })
            }
        })
        .build(&l2_trace)?;

    // TODO: change to Result::inspect_err when sp1 toolchain >= 1.76
    #[allow(clippy::map_identity)]
    executor.handle_block(&l2_trace).map_err(|e| {
        dev_error!(
            "Error occurs when executing block {:?}: {e:?}",
            l2_trace.header.hash.unwrap()
        );

        update_metrics_counter!(verification_error);
        e
    })?;
    println!("start commit_changes for block");
    let revm_root_after = executor.commit_changes(&mut zktrie_state);
    println!("end commit_changes for block");

    #[cfg(feature = "profiling")]
    if let Ok(report) = guard.report().build() {
        let dir = std::env::temp_dir()
            .join(env!("CARGO_PKG_NAME"))
            .join("profiling");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!(
            "block-{}.svg",
            l2_trace.header.number.unwrap().as_u64()
        ));
        let file = std::fs::File::create(&path).unwrap();
        report.flamegraph(file).unwrap();
        dev_info!("Profiling report saved to: {:?}", path);
    }

    if root_after != revm_root_after {
        dev_error!(
            "Block #{}({:?}) root mismatch: root after in trace = {root_after:x}, root after in revm = {revm_root_after:x}",
            l2_trace.header.number.unwrap().as_u64(),
            l2_trace.header.hash.unwrap()
        );

        update_metrics_counter!(verification_error);

        return Err(VerificationError::RootMismatch {
            root_trace: root_after,
            root_revm: revm_root_after,
        });
    }
    dev_info!(
        "Block #{}({}) verified successfully",
        l2_trace.header.number.unwrap().as_u64(),
        l2_trace.header.hash.unwrap()
    );
    Ok(())
}

pub const KECCAK_EMPTY: B256 =
    b256!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");

/**
 * exec
 */
pub fn exec(n: u32) -> (u32, u32) {
    for _ in 0..20 {
        let cache_state = revm::CacheState::new(false);

        let acc_info = revm::primitives::AccountInfo {
            balance: U256::from(10u64.pow(18)),
            #[cfg(feature = "scroll")]
            code_size,
            code_hash: KECCAK_EMPTY,
            #[cfg(feature = "scroll-poseidon-codehash")]
            poseidon_code_hash,
            code: None,
            nonce: 0,
            code_size: todo!(),
            poseidon_code_hash: todo!(),
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
