use crate::{
    error::ZkTrieError, executor::hooks::ExecuteHooks, BlockTraceExt, EvmExecutor, ReadOnlyDB,
};
use mpt_zktrie::ZktrieState;
use revm::{db::CacheDB, primitives::SpecId};

/// Builder for EVM executor.
#[derive(Debug)]
pub struct EvmExecutorBuilder<'a> {
    execute_hooks: ExecuteHooks,
    zktrie_state: &'a ZktrieState,
}

impl<'a> EvmExecutorBuilder<'a> {
    /// Create a new builder.
    pub fn new(zktrie_state: &'a ZktrieState) -> Self {
        Self { execute_hooks: ExecuteHooks::default(), zktrie_state }
    }
}

impl<'a> EvmExecutorBuilder<'a> {
    /// Set hardfork config.
    pub fn hardfork_config(self) -> EvmExecutorBuilder<'a> {
        EvmExecutorBuilder { execute_hooks: self.execute_hooks, zktrie_state: self.zktrie_state }
    }

    /// Modify execute hooks.
    pub fn with_execute_hooks(mut self, modify: impl FnOnce(&mut ExecuteHooks)) -> Self {
        modify(&mut self.execute_hooks);
        self
    }

    /// Set zktrie state.
    pub fn zktrie_state(self, zktrie_state: &ZktrieState) -> EvmExecutorBuilder {
        EvmExecutorBuilder { zktrie_state, ..self }
    }
}

impl<'a> EvmExecutorBuilder<'a> {
    /// Initialize an EVM executor from a block trace as the initial state.
    pub fn build<T: BlockTraceExt>(self, l2_trace: &'a T) -> Result<EvmExecutor, ZkTrieError> {
        let spec_id = SpecId::CURIE;

        dev_trace!("use spec id {:?}", spec_id);

        let db = cycle_track!(
            CacheDB::new(ReadOnlyDB::new(l2_trace, self.zktrie_state)?),
            "build ReadOnlyDB"
        );

        Ok(EvmExecutor { db, spec_id, hooks: self.execute_hooks })
    }
}
