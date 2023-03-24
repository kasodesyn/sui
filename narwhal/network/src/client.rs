// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anemo::{PeerId, Request};
use async_trait::async_trait;
use crypto::{NetworkKeyPair, NetworkPublicKey};
use parking_lot::RwLock;
use tokio::time::sleep;
use types::{
    error::LocalClientError, PrimaryToWorker, WorkerSynchronizeMessage, WorkerToPrimary,
    WorkerToWorker,
};

use crate::traits::PrimaryToOwnWorkerClient;

#[derive(Clone)]
pub struct NetworkClient {
    inner: Arc<RwLock<Inner>>,
}

struct Inner {
    // The private-public network key pair of this authority.
    _primary_network_keypair: NetworkKeyPair,
    worker_to_primary_handle: Option<Arc<dyn WorkerToPrimary>>,
    primary_to_own_worker_handler: BTreeMap<PeerId, Arc<dyn PrimaryToWorker>>,
    worker_to_own_worker_handler: BTreeMap<PeerId, Arc<dyn WorkerToWorker>>,
    shutdown: bool,
}

impl NetworkClient {
    pub fn new(primary_network_keypair: NetworkKeyPair) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                _primary_network_keypair: primary_network_keypair,
                worker_to_primary_handle: None,
                primary_to_own_worker_handler: BTreeMap::new(),
                worker_to_own_worker_handler: BTreeMap::new(),
                shutdown: false,
            })),
        }
    }

    pub fn set_worker_to_primary_local_handler(&self, server: Arc<dyn WorkerToPrimary>) {
        let mut inner = self.inner.write();
        inner.worker_to_primary_handle = Some(server);
    }

    pub fn set_primary_to_worker_local_handler(
        &self,
        worker_id: PeerId,
        server: Arc<dyn PrimaryToWorker>,
    ) {
        let mut inner = self.inner.write();
        inner
            .primary_to_own_worker_handler
            .insert(worker_id, server);
    }

    pub fn set_worker_to_worker_local_handler(
        &self,
        worker_id: PeerId,
        server: Arc<dyn WorkerToWorker>,
    ) {
        let mut inner = self.inner.write();
        inner.worker_to_own_worker_handler.insert(worker_id, server);
    }

    pub fn shutdown(&self) {
        let mut inner = self.inner.write();
        inner.shutdown = true;
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

    async fn _get_worker_to_own_worker_handler(
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
