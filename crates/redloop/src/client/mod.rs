//! High-level Redloop client handles.

mod counts;
mod jobs;
mod namespaces;
mod workers;

use std::sync::Arc;

use crate::clock::{Clock, WorkerCoordinator};
use crate::store::QueueStore;

/// Top-level Redloop client.
#[derive(Clone)]
pub struct Redloop {
    pub(crate) store: Arc<dyn QueueStore>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) coordinator: Arc<dyn WorkerCoordinator>,
}

/// Namespace-scoped queue handle.
#[derive(Clone)]
pub struct Namespace {
    pub(crate) store: Arc<dyn QueueStore>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) coordinator: Arc<dyn WorkerCoordinator>,
    pub(crate) namespace: String,
}

#[cfg(test)]
#[path = "_tests_/client_test_support.rs"]
pub(crate) mod client_test_support;
