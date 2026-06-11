//! Worker heartbeat lease maintenance.

use std::collections::HashMap;

use crate::clock::Clock;
use crate::config::WorkerConfig;
use crate::error::{Error, Result};
use crate::store::QueueStore;

use super::leases::{ActiveLease, LeaseAttemptKey};

pub(crate) async fn heartbeat_active(
    namespace: &str,
    store: &dyn QueueStore,
    clock: &dyn Clock,
    config: &WorkerConfig,
    active: &mut HashMap<LeaseAttemptKey, ActiveLease>,
) -> Result<()> {
    let now = clock.now();
    let leases = active
        .values()
        .filter(|lease| lease.heartbeat_enabled())
        .map(|lease| {
            (
                lease.attempt_key(),
                lease.job_id.clone(),
                lease.lease_token.clone(),
            )
        })
        .collect::<Vec<_>>();

    for (attempt_key, job_id, lease_token) in leases {
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
                    "worker heartbeat lost lease; marking local attempt as lost"
                );
                if let Some(lease) = active.get_mut(&attempt_key) {
                    lease.disable_heartbeat();
                }
            }
            Err(error) => return Err(error),
        }
    }

    Ok(())
}
