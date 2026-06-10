//! Worker task spawning helpers.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::task::JoinSet;

use crate::store::ReservedJob;

use super::handler::RuntimeHandler;
use super::leases::{ActiveLease, CompletedLease};

pub(crate) fn spawn_job(
    join_set: &mut JoinSet<CompletedLease>,
    active: &mut HashMap<String, ActiveLease>,
    handler: Arc<dyn RuntimeHandler>,
    lease: ReservedJob,
) {
    let job_id = lease.job_id.clone();
    active.insert(
        job_id.clone(),
        ActiveLease {
            job_id: lease.job_id.clone(),
            lease_token: lease.lease_token.clone(),
        },
    );

    join_set.spawn(async move {
        let result = handler.handle(lease.job_id.clone()).await;
        CompletedLease {
            job_id: lease.job_id,
            lease_token: lease.lease_token,
            result,
        }
    });
}
