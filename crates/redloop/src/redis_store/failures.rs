//! Operator-driven queue mutation operations.

use crate::config::Timestamp;
use crate::error::Result;
use crate::types::ForceFailRequest;

use super::RedisStore;
use super::scripts;
use super::shared::{micros, parse_script_error};

impl RedisStore {
    pub(crate) async fn cancel(&self, namespace: &str, job_id: &str) -> Result<()> {
        operator_transition(self, namespace, "cancel", job_id).await
    }

    pub(crate) async fn force_ack(&self, namespace: &str, job_id: &str) -> Result<()> {
        operator_transition(self, namespace, "force_ack", job_id).await
    }

    pub(crate) async fn force_fail(
        &self,
        namespace: &str,
        request: ForceFailRequest,
        now: Timestamp,
    ) -> Result<()> {
        let _ = request.message;
        operator_transition_at(self, namespace, "force_fail", &request.job_id, now).await
    }

    pub(crate) async fn requeue(
        &self,
        namespace: &str,
        job_id: &str,
        now: Timestamp,
    ) -> Result<()> {
        operator_transition_at(self, namespace, "requeue", job_id, now).await
    }

    pub(crate) async fn retry_now(
        &self,
        namespace: &str,
        job_id: &str,
        now: Timestamp,
    ) -> Result<()> {
        operator_transition_at(self, namespace, "retry_now", job_id, now).await
    }

    pub(crate) async fn force_retry(
        &self,
        namespace: &str,
        job_id: &str,
        now: Timestamp,
    ) -> Result<()> {
        operator_transition_at(self, namespace, "force_retry", job_id, now).await
    }
}

async fn operator_transition(
    store: &RedisStore,
    namespace: &str,
    op: &str,
    job_id: &str,
) -> Result<()> {
    operator_transition_at(
        store,
        namespace,
        op,
        job_id,
        Timestamp::from(std::time::SystemTime::now()),
    )
    .await
}

async fn operator_transition_at(
    store: &RedisStore,
    namespace: &str,
    op: &str,
    job_id: &str,
    now: Timestamp,
) -> Result<()> {
    let keys = store.keys(namespace);
    let response = store
        .eval(
            "operator_transition",
            scripts::OPERATOR_TRANSITION,
            &[
                keys.failures,
                keys.ready,
                keys.scheduled,
                keys.leased,
                keys.lease_meta,
                keys.failed,
                keys.workers_leases,
                keys.rerun,
            ],
            &[op.to_string(), job_id.to_string(), micros(now).to_string()],
        )
        .await?;
    if response.first().map(String::as_str) == Some("ok") {
        Ok(())
    } else {
        Err(parse_script_error(job_id, &response))
    }
}
