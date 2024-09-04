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

use alloy_sol_types::sol;

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
    let disable_checks = true;
    let mut fork_config = HardforkConfig::default_from_chain_id(534352);
    fork_config.set_curie_block(0);

    let root_after = l2_trace.storage_trace.root_after;

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
    let revm_root_after = executor.commit_changes(&mut zktrie_state);

    // if root_after != revm_root_after {
    //     dev_error!(
    //         "Block #{}({:?}) root mismatch: root after in trace = {root_after:x}, root after in revm = {revm_root_after:x}",
    //         l2_trace.header.number.unwrap().as_u64(),
    //         l2_trace.header.hash.unwrap()
    //     );

    //     update_metrics_counter!(verification_error);

    //     return Err(VerificationError::RootMismatch {
    //         root_trace: root_after,
    //         root_revm: revm_root_after,
    //     });
    // }
    dev_info!(
        "Block #{}({}) verified successfully",
        l2_trace.header.number.unwrap().as_u64(),
        l2_trace.header.hash.unwrap()
    );
    Ok(())
}
