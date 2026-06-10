//! Worker heartbeat lease maintenance.

use std::collections::HashMap;

use crate::clock::Clock;
use crate::config::WorkerConfig;
use crate::error::{Error, Result};
use crate::store::QueueStore;

use super::leases::ActiveLease;

pub(crate) async fn heartbeat_active(
    namespace: &str,
    store: &dyn QueueStore,
    clock: &dyn Clock,
    config: &WorkerConfig,
    active: &mut HashMap<String, ActiveLease>,
) -> Result<()> {
    let now = clock.now();
    let leases = active
        .values()
        .map(|lease| (lease.job_id.clone(), lease.lease_token.clone()))
        .collect::<Vec<_>>();

    for (job_id, lease_token) in leases {
        match store
            .heartbeat(
                namespace,
                &config.worker_id,
                &job_id,
                &lease_token,
                now,
                config.lease_duration,
            )
            .await
        {
            Ok(()) => {}
            Err(Error::LeaseMismatch {
                job_id: lease_job_id,
            }) => {
                tracing::warn!(
                    job_id = %job_id,
                    lease_mismatch_job_id = %lease_job_id,
                    "worker heartbeat lost lease; dropping local lease attempt"
                );
                active.remove(&job_id);
            }
            Err(error) => return Err(error),
        }
    }

    Ok(())
}
