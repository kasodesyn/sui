// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anemo::{PeerId, Request};
use async_trait::async_trait;
use crypto::{traits::KeyPair, NetworkKeyPair, NetworkPublicKey};
use parking_lot::RwLock;
use tokio::time::sleep;
use tracing::debug;
use types::{
    error::LocalClientError, PrimaryToWorker, WorkerOthersBatchMessage, WorkerOurBatchMessage,
    WorkerSynchronizeMessage, WorkerToPrimary, WorkerToWorker,
};

use crate::traits::{PrimaryToOwnWorkerClient, WorkerToOwnPrimaryClient};

#[derive(Clone)]
pub struct NetworkClient {
    inner: Arc<RwLock<Inner>>,
}

struct Inner {
    // The private-public network key pair of this authority.
    primary_peer_id: PeerId,
    worker_to_primary_handler: Option<Arc<dyn WorkerToPrimary>>,
    primary_to_own_worker_handler: BTreeMap<PeerId, Arc<dyn PrimaryToWorker>>,
    worker_to_own_worker_handler: BTreeMap<PeerId, Arc<dyn WorkerToWorker>>,
    shutdown: bool,
}

impl NetworkClient {
    pub fn new(primary_peer_id: PeerId) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                primary_peer_id,
                worker_to_primary_handler: None,
                primary_to_own_worker_handler: BTreeMap::new(),
                worker_to_own_worker_handler: BTreeMap::new(),
                shutdown: false,
            })),
        }
    }

    pub fn new_from_keypair(primary_network_keypair: &NetworkKeyPair) -> Self {
        Self::new(PeerId(primary_network_keypair.public().0.into()))
    }

    pub fn new_with_empty_id() -> Self {
        // ED25519_PUBLIC_KEY_LENGTH is 32 bytes.
        Self::new(empty_peer_id())
    }

    pub fn set_worker_to_primary_local_handler(&self, handler: Arc<dyn WorkerToPrimary>) {
        let mut inner = self.inner.write();
        inner.worker_to_primary_handler = Some(handler);
    }

    pub fn set_primary_to_worker_local_handler(
        &self,
        worker_id: PeerId,
        handler: Arc<dyn PrimaryToWorker>,
    ) {
        let mut inner = self.inner.write();
        inner
            .primary_to_own_worker_handler
            .insert(worker_id, handler);
    }

    pub fn set_worker_to_worker_local_handler(
        &self,
        worker_id: PeerId,
        handler: Arc<dyn WorkerToWorker>,
    ) {
        let mut inner = self.inner.write();
        inner
            .worker_to_own_worker_handler
            .insert(worker_id, handler);
    }

    pub fn shutdown(&self) {
        let mut inner = self.inner.write();
        if inner.shutdown {
            return;
        }
        // Clears internal data and sets shutdown flag.
        *inner = Inner {
            primary_peer_id: inner.primary_peer_id,
            worker_to_primary_handler: None,
            primary_to_own_worker_handler: BTreeMap::new(),
            worker_to_own_worker_handler: BTreeMap::new(),
            shutdown: true,
        };
    }

    async fn get_primary_to_own_worker_handler(
        &self,
        peer_id: PeerId,
    ) -> Result<Arc<dyn PrimaryToWorker>, LocalClientError> {
        for _ in 0..10 {
            {
                let inner = self.inner.read();
                if inner.shutdown {
                    return Err(LocalClientError::ShuttingDown);
                }
                if let Some(handler) = inner.primary_to_own_worker_handler.get(&peer_id) {
                    return Ok(handler.clone());
                }
            }
            sleep(Duration::from_millis(500)).await;
        }
        Err(LocalClientError::WorkerNotStarted(peer_id))
    }

    async fn get_worker_to_own_primary_handler(
        &self,
    ) -> Result<Arc<dyn WorkerToPrimary>, LocalClientError> {
        for _ in 0..10 {
            {
                let inner = self.inner.read();
                if inner.shutdown {
                    return Err(LocalClientError::ShuttingDown);
                }
                if let Some(handler) = &inner.worker_to_primary_handler {
                    debug!("Found primary {}", inner.primary_peer_id);
                    return Ok(handler.clone());
                }
            }
            sleep(Duration::from_millis(500)).await;
        }
        Err(LocalClientError::PrimaryNotStarted(
            self.inner.read().primary_peer_id,
        ))
    }

    async fn _get_own_worker_to_worker_handler(
        &self,
        peer_id: PeerId,
    ) -> Result<Arc<dyn WorkerToWorker>, LocalClientError> {
        for _ in 0..10 {
            {
                let inner = self.inner.read();
                if inner.shutdown {
                    return Err(LocalClientError::ShuttingDown);
                }
                if let Some(handler) = inner.worker_to_own_worker_handler.get(&peer_id) {
                    return Ok(handler.clone());
                }
            }
            sleep(Duration::from_millis(500)).await;
        }
        Err(LocalClientError::WorkerNotStarted(peer_id))
    }
}

#[async_trait]
impl PrimaryToOwnWorkerClient for NetworkClient {
    async fn synchronize(
        &self,
        worker_peer: NetworkPublicKey,
        message: WorkerSynchronizeMessage,
    ) -> Result<(), LocalClientError> {
        let c = self
            .get_primary_to_own_worker_handler(PeerId(worker_peer.0.into()))
            .await?;
        c.synchronize(Request::new(message))
            .await
            .map_err(|e| LocalClientError::Internal(format!("{e:?}")))?;
        Ok(())
    }
}

#[async_trait]
impl WorkerToOwnPrimaryClient for NetworkClient {
    async fn report_our_batch(
        &self,
        request: WorkerOurBatchMessage,
    ) -> Result<(), LocalClientError> {
        let c = self.get_worker_to_own_primary_handler().await?;
        c.report_our_batch(Request::new(request))
            .await
            .map_err(|e| LocalClientError::Internal(format!("{e:?}")))?;
        Ok(())
    }

    async fn report_others_batch(
        &self,
        request: WorkerOthersBatchMessage,
    ) -> Result<(), LocalClientError> {
        let c = self.get_worker_to_own_primary_handler().await?;
        c.report_others_batch(Request::new(request))
            .await
            .map_err(|e| LocalClientError::Internal(format!("{e:?}")))?;
        Ok(())
    }
}

fn empty_peer_id() -> PeerId {
    PeerId([0u8; 32])
}
