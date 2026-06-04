//! Test helpers for the client module.

use std::sync::Arc;

use unimock::Unimock;

use crate::clock::{SystemClock, TokioWorkerCoordinator};

pub(crate) fn namespace_for_tests(namespace: String) -> super::Namespace {
    let client = super::Redloop {
        store: Arc::new(Unimock::new(())),
        clock: Arc::new(SystemClock),
        coordinator: Arc::new(TokioWorkerCoordinator),
    };
    client.namespace_handle(namespace)
}
