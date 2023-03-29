// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
use anyhow::{bail, Result};
use axum::{extract::Extension, http::StatusCode, routing::get, Router};
use mysten_metrics::RegistryService;
use prometheus::{
    proto::{Metric, MetricFamily},
    TextEncoder,
};
use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::{Arc, RwLock},
};
use tracing::warn;

const METRICS_ROUTE: &str = "/metrics";

// Creates a new http server that has as a sole purpose to expose
// and endpoint that prometheus agent can use to poll for the metrics.
// A RegistryService is returned that can be used to get access in prometheus Registries.
pub fn start_prometheus_server(addr: SocketAddr) -> HistogramRelay {
    let relay = HistogramRelay::new();
    let app = Router::new()
        .route(METRICS_ROUTE, get(metrics))
        .layer(Extension(relay.clone()));

    tokio::spawn(async move {
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
    });
    relay
}

async fn metrics(Extension(relay): Extension<HistogramRelay>) -> (StatusCode, String) {
    let Ok(expformat) = relay.export() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("unable to pop metrics from HistogramRelay"),
        );
    };
    (StatusCode::OK, expformat)
}

#[derive(Clone)]
pub struct HistogramRelay(Arc<RwLock<VecDeque<Vec<MetricFamily>>>>);

impl HistogramRelay {
    pub fn new() -> Self {
        HistogramRelay(Arc::new(RwLock::new(VecDeque::new())))
    }
    pub fn submit(&self, data: Vec<MetricFamily>) {
        self.0
            .write()
            .expect("couldn't get mut lock on HistogramRelay")
            .push_back(data);
    }
    pub fn export(&self) -> Result<String> {
        let Some(data) = self
            .0
            .write()
            .expect("couldn't get mut lock on HistogramRelay")
            .pop_front() else {
                warn!("no data in HistogramRelay buffer, this may be ok...");
                bail!("no data in HistogramRelay to scrape")
            };
        let histograms: Vec<MetricFamily> = extract_histograms(data).collect();
        let encoder = prometheus::TextEncoder::new();
        let string = match encoder.encode_to_string(&histograms) {
            Ok(s) => s,
            Err(error) => bail!("{error}"),
        };
        Ok(string)
    }
}

fn extract_histograms(data: Vec<MetricFamily>) -> impl Iterator<Item = MetricFamily> {
    data.into_iter().map(|mf| {
        let metrics = mf.get_metric().iter().map(|m| {
            let mut v = Metric::default();
            v.set_label(protobuf::RepeatedField::from_slice(m.get_label()));
            v.set_histogram(m.get_histogram().to_owned());
            v.set_timestamp_ms(m.get_timestamp_ms());
            v
        });
        let mut v = MetricFamily::default();
        v.set_name(mf.get_name().to_owned());
        v.set_help(mf.get_help().to_owned());
        v.set_field_type(mf.get_field_type());
        v.set_metric(protobuf::RepeatedField::from_iter(metrics));
        v
    })
}
