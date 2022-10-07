use massa_models::{
    block::BlockId, endorsement::EndorsementId, operation::OperationId, slot::Slot,
};
use massa_pool_exports::{PoolConfig, PoolController, PoolError, PoolManager};
use massa_storage::Storage;
use parking_lot::RwLock;
use std::sync::{mpsc::SyncSender, Arc};
use tracing::{info, warn};

use crate::{endorsement_pool::EndorsementPool, operation_pool::OperationPool};

pub enum Command {
    AddOperations(Storage),
    AddEndorsements(Storage),
    NotifyFinalCsPeriods(Vec<u64>),
    Stop,
}

#[derive(Clone)]
pub struct PoolControllerImpl {
    pub(crate) _config: PoolConfig,
    pub(crate) operation_pool: Arc<RwLock<OperationPool>>,
    pub(crate) endorsement_pool: Arc<RwLock<EndorsementPool>>,
    pub(crate) operations_input_sender: SyncSender<Command>,
    pub(crate) endorsements_input_sender: SyncSender<Command>,
}

impl PoolController for PoolControllerImpl {
    /// add operations to pool
    fn add_operations(&mut self, ops: Storage) -> Result<(), PoolError> {
        // self.operation_pool.write().add_operations(ops);
        // TODO: DROP TYPE CHANNEL
        self.operations_input_sender
            .send(Command::AddOperations(ops))
            .map_err(|_err| {
                PoolError::ChannelError(
                    "could not give operations to add through pool channel".into(),
                )
            })?;
        Ok(())
    }

    /// add endorsements to pool
    fn add_endorsements(&mut self, endorsements: Storage) -> Result<(), PoolError> {
        // self.endorsement_pool.write().add_endorsements(endorsements);
        self.endorsements_input_sender
            .send(Command::AddEndorsements(endorsements))
            .map_err(|_err| {
                PoolError::ChannelError(
                    "could not give endorsements to add through pool channel".into(),
                )
            })?;
        Ok(())
    }

    /// notify of new final consensus periods (1 per thread)
    fn notify_final_cs_periods(&mut self, final_cs_periods: &[u64]) -> Result<(), PoolError> {
        // self.operation_pool
        //     .write()
        //     .notify_final_cs_periods(final_cs_periods);
        // self.endorsement_pool
        //     .write()
        //     .notify_final_cs_periods(final_cs_periods);
        self.operations_input_sender
            .send(Command::NotifyFinalCsPeriods(final_cs_periods.to_vec()))
            .map_err(|_err| {
                PoolError::ChannelError(
                    "could not give consensus periods through pool channel".into(),
                )
            })?;
        self.endorsements_input_sender
            .send(Command::NotifyFinalCsPeriods(final_cs_periods.to_vec()))
            .map_err(|_err| {
                PoolError::ChannelError(
                    "could not give consensus periodsthrough pool channel".into(),
                )
            })?;
        Ok(())
    }

    /// get operations for block creation
    fn get_block_operations(&self, slot: &Slot) -> (Vec<OperationId>, Storage) {
        self.operation_pool.read().get_block_operations(slot)
    }

    /// get endorsements for a block
    fn get_block_endorsements(
        &self,
        target_block: &BlockId,
        target_slot: &Slot,
    ) -> (Vec<Option<EndorsementId>>, Storage) {
        self.endorsement_pool
            .read()
            .get_block_endorsements(target_slot, target_block)
    }

    /// Returns a boxed clone of self.
    /// Allows cloning `Box<dyn PoolController>`,
    fn clone_box(&self) -> Box<dyn PoolController> {
        Box::new(self.clone())
    }

    /// Get the number of endorsements in the pool
    fn get_endorsement_count(&self) -> usize {
        self.endorsement_pool.read().len()
    }

    /// Get the number of operations in the pool
    fn get_operation_count(&self) -> usize {
        self.operation_pool.read().len()
    }

    /// Check if the pool contains a list of endorsements. Returns one boolean per item.
    fn contains_endorsements(&self, endorsements: &[EndorsementId]) -> Vec<bool> {
        let lck = self.endorsement_pool.read();
        endorsements.iter().map(|id| lck.contains(id)).collect()
    }

    /// Check if the pool contains a list of operations. Returns one boolean per item.
    fn contains_operations(&self, operations: &[OperationId]) -> Vec<bool> {
        let lck = self.operation_pool.read();
        operations.iter().map(|id| lck.contains(id)).collect()
    }
}

/// TODO
pub struct PoolManagerImpl {
    /// Handle used to join the operation thread
    pub(crate) operations_thread_handle: Option<std::thread::JoinHandle<Result<(), PoolError>>>,
    /// Handle used to join the endorsement thread
    pub(crate) endorsements_thread_handle: Option<std::thread::JoinHandle<Result<(), PoolError>>>,
    /// Operations input data mpsc (used to stop the pool thread)
    pub(crate) operations_input_sender: SyncSender<Command>,
    /// Endorsements input data mpsc (used to stop the pool thread)
    pub(crate) endorsements_input_sender: SyncSender<Command>,
}

impl PoolManager for PoolManagerImpl {
    /// stops the worker
    fn stop(&mut self) {
        info!("stopping pool worker...");
        let _ = self.operations_input_sender.send(Command::Stop);
        let _ = self.endorsements_input_sender.send(Command::Stop);
        if let Some(join_handle) = self.operations_thread_handle.take() {
            if let Err(err) = join_handle
                .join()
                .expect("operations pool thread panicked on try to join")
            {
                warn!("{}", err);
            }
        }
        if let Some(join_handle) = self.endorsements_thread_handle.take() {
            if let Err(err) = join_handle
                .join()
                .expect("endorsements pool thread panicked on try to join")
            {
                warn!("{}", err);
            }
        }
        info!("pool worker stopped");
    }
}
