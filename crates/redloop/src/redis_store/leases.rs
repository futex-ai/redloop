//! Lease lifecycle operations for active workers.

use crate::config::Timestamp;
use crate::error::{Error, Result};
use crate::store::FailureRequest;

use super::RedisStore;
use super::scripts;
use super::shared::{
    failure_args, from_micros, micros, parse_i64, parse_lease_response, parse_script_error,
    parse_state,
};

impl RedisStore {
    pub(crate) async fn heartbeat(
        &self,
        namespace: &str,
        worker_id: &str,
        job_id: &str,
        lease_token: &str,
        now: Timestamp,
        lease_duration: std::time::Duration,
    ) -> Result<()> {
        let keys = self.keys(namespace);
        let response = self
            .eval(
                "heartbeat",
                scripts::HEARTBEAT,
                &[keys.leased, keys.lease_meta, keys.workers_last_seen],
                &[
                    job_id.to_string(),
                    worker_id.to_string(),
                    lease_token.to_string(),
                    micros(now).to_string(),
                    micros(now)
                        .saturating_add(super::shared::duration_micros(lease_duration))
                        .to_string(),
                ],
            )
            .await?;
        parse_lease_response(job_id, response)
    }

    pub(crate) async fn ack(
        &self,
        namespace: &str,
        worker_id: &str,
        job_id: &str,
        lease_token: &str,
    ) -> Result<()> {
        let keys = self.keys(namespace);
        let now = Timestamp::from(std::time::SystemTime::now());
        let response = self
            .eval(
                "ack",
                scripts::ACK,
                &[
                    keys.leased,
                    keys.lease_meta,
                    keys.failures,
                    keys.workers_leases,
                    keys.ready,
                    keys.rerun,
                ],
                &[
                    job_id.to_string(),
                    worker_id.to_string(),
                    lease_token.to_string(),
                    micros(now).to_string(),
                ],
            )
            .await?;
        parse_lease_response(job_id, response)
    }

    pub(crate) async fn complete_and_reschedule(
        &self,
        namespace: &str,
        worker_id: &str,
        job_id: &str,
        lease_token: &str,
        schedule_at: Option<Timestamp>,
        now: Timestamp,
    ) -> Result<()> {
        let keys = self.keys(namespace);
        let response = self
            .eval(
                "complete_and_reschedule",
                scripts::COMPLETE_AND_RESCHEDULE,
                &[
                    keys.leased,
                    keys.lease_meta,
                    keys.failures,
                    keys.workers_leases,
                    keys.ready,
                    keys.scheduled,
                    keys.rerun,
                ],
                &[
                    job_id.to_string(),
                    worker_id.to_string(),
                    lease_token.to_string(),
                    micros(now).to_string(),
                    schedule_at.map(micros).unwrap_or_default().to_string(),
                ],
            )
            .await?;
        parse_lease_response(job_id, response)
    }

    pub(crate) async fn fail_or_retry(
        &self,
        namespace: &str,
        request: FailureRequest,
    ) -> Result<u32> {
        let keys = self.keys(namespace);
        let response = self
            .eval(
                "fail_or_retry",
                scripts::FAIL_OR_RETRY,
                &[
                    keys.leased,
                    keys.lease_meta,
                    keys.failures,
                    keys.ready,
                    keys.scheduled,
                    keys.failed,
                    keys.workers_leases,
                    keys.rerun,
                ],
                &failure_args(&request),
            )
            .await?;

        if response.first().map(String::as_str) != Some("ok") {
            return Err(parse_script_error(&request.job_id, &response));
        }

        let failure_count = response
            .get(1)
            .ok_or(Error::InvalidData {
                field: "fail_or_retry.failure_count",
            })?
            .parse::<u32>()
            .map_err(|_| Error::InvalidData {
                field: "fail_or_retry.failure_count",
            })?;
        let _ = parse_state(response.get(2).ok_or(Error::InvalidData {
            field: "fail_or_retry.state",
        })?)?;
        if let Some(value) = response.get(3).map(String::as_str)
            && value != "0"
        {
            let _ = from_micros(parse_i64("fail_or_retry.schedule_at", value)?)?;
        }

        Ok(failure_count)
    }

    pub(crate) async fn reap_expired(
        &self,
        namespace: &str,
        now: Timestamp,
        limit: usize,
    ) -> Result<u64> {
        if limit == 0 {
            return Ok(0);
        }

        let keys = self.keys(namespace);
        let response = self
            .eval(
                "reap_expired",
                scripts::REAP_EXPIRED,
                &[
                    keys.leased,
                    keys.lease_meta,
                    keys.ready,
                    keys.workers_leases,
                    keys.rerun,
                ],
                &[micros(now).to_string(), limit.to_string()],
            )
            .await?;
        if response.first().map(String::as_str) != Some("ok") {
            return Err(parse_script_error(namespace, &response));
        }
        response
            .get(1)
            .ok_or(Error::InvalidData {
                field: "reap_expired.count",
            })?
            .parse::<u64>()
            .map_err(|_| Error::InvalidData {
                field: "reap_expired.count",
            })
    }
}
