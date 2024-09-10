use std::rc::Rc;

use alloy::primitives::keccak256;
use sbv_core::{EvmExecutorBuilder, HardforkConfig, VerificationError};
use sbv_primitives::{types::BlockTrace, zk_trie::ZkMemoryDb, Block, B256};
use sbv_utils::{
    cycle_track, dev_error, dev_info, dev_trace, measure_duration_histogram, update_metrics_counter,
};

pub fn verify(l2_trace: &BlockTrace) -> Result<B256, VerificationError> {
    dev_trace!("{l2_trace:#?}");

    // Step1: verify blob
    // TODO
    let versioned_hash: Vec<u8> = vec![];

    // Step2: Execute all transactions and get stateroot
    let root_before = l2_trace.root_before();
    let root_after = l2_trace.root_after();

    let fork_config: HardforkConfig = HardforkConfig::default_from_chain_id(2818);

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

    let zktrie_db = cycle_track!(
        {
            let mut zktrie_db = ZkMemoryDb::new();
            measure_duration_histogram!(
                build_zktrie_db_duration_microseconds,
                l2_trace.build_zktrie_db(&mut zktrie_db)
            );
            Rc::new(zktrie_db)
        },
        "build ZktrieState"
    );

    let mut executor = EvmExecutorBuilder::new(zktrie_db.clone())
        .hardfork_config(fork_config)
        .build(&l2_trace)?;

    // TODO: change to Result::inspect_err when sp1 toolchain >= 1.76
    #[allow(clippy::map_identity)]
    #[allow(clippy::manual_inspect)]
    executor.handle_block(&l2_trace).map_err(|e| {
        dev_error!(
            "Error occurs when executing block #{}({:?}): {e:?}",
            l2_trace.number(),
            l2_trace.block_hash()
        );

        update_metrics_counter!(verification_error);
        e
    })?;
    let revm_root_after = executor.commit_changes(&zktrie_db);

    #[cfg(feature = "profiling")]
    if let Ok(report) = guard.report().build() {
        let dir = std::env::temp_dir()
            .join(env!("CARGO_PKG_NAME"))
            .join("profiling");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("block-{}.svg", l2_trace.number()));
        let file = std::fs::File::create(&path).unwrap();
        report.flamegraph(file).unwrap();
        dev_info!("Profiling report saved to: {:?}", path);
    }

    if root_after != revm_root_after {
        dev_error!(
            "Block #{}({:?}) root mismatch: root after in trace = {root_after:x}, root after in
    revm = {revm_root_after:x}",
            l2_trace.number(),
            l2_trace.block_hash(),
        );

        update_metrics_counter!(verification_error);

        return Err(VerificationError::RootMismatch {
            root_trace: root_after,
            root_revm: revm_root_after,
        });
    }
    dev_info!(
        "Block #{}({}) verified successfully",
        l2_trace.number(),
        l2_trace.block_hash(),
    );
    let pi_hash = keccak256(
        [
            versioned_hash,
            root_before.to_vec(),
            revm_root_after.to_vec(),
        ]
        .concat(),
    );

    Ok(B256::from_slice(pi_hash.as_slice()))
}
