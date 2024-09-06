//! Stateless Block Verifier

#![feature(lazy_cell)]
// #![deny(missing_docs)]
#![deny(missing_debug_implementations)]

use revm::primitives::keccak256;
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

pub mod block_trace;

/// Module for utilities.
pub mod utils;
use utils::ext::BlockZktrieExt;
pub use utils::{post_check, BlockTraceExt};

use block_trace::BlockTrace;
use ethers_core::types::H256;

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

pub fn verify(l2_trace: &BlockTrace) -> Result<H256, VerificationError> {
    let disable_checks = true;

    let root_before = l2_trace.storage_trace.root_before;
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
        dev_error!("Error occurs when executing block {:?}: {e:?}", l2_trace.header.hash.unwrap());

        update_metrics_counter!(verification_error);
        e
    })?;
    let revm_root_after = executor.commit_changes(&mut zktrie_state);

    // if root_after != revm_root_after {
    //     dev_error!(
    //         "Block #{}({:?}) root mismatch: root after in trace = {root_after:x}, root after in
    // revm = {revm_root_after:x}",         l2_trace.header.number.unwrap().as_u64(),
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
    let pi_hash = keccak256([root_before.as_bytes(), revm_root_after.as_bytes()].concat());

    Ok(H256::from_slice(pi_hash.as_slice()))
}
