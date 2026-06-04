//! Enqueue, reschedule, and job-loading operations.

use redis::pipe;

use crate::config::Timestamp;
use crate::error::{Error, Result};
use crate::store::EnqueueRequest;
use crate::types::{EnqueueResult, JobRecord, JobState, LeaseHolder, RescheduleResult};

use super::RedisStore;
use super::scripts;
use super::shared::{
    from_micros, micros, parse_enqueue_status, parse_i64, parse_script_error, parse_state,
    replace_mode,
};

type JobLookupFields = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<u32>,
    Option<String>,
);

impl RedisStore {
    pub(crate) async fn enqueue(
        &self,
        namespace: &str,
        request: EnqueueRequest,
    ) -> Result<EnqueueResult> {
        let keys = self.keys(namespace);
        let now = Timestamp::from(std::time::SystemTime::now());
        let response = self
            .eval(
                "enqueue_or_schedule",
                scripts::ENQUEUE_OR_RESCHEDULE,
                &[
                    keys.failures,
                    keys.ready,
                    keys.scheduled,
                    keys.leased,
                    keys.failed,
                    keys.rerun,
                ],
                &[
                    if request.allow_existing_current_run {
                        "enqueue_immediate".to_string()
                    } else {
                        "enqueue_scheduled".to_string()
                    },
                    request.job_id.clone(),
                    micros(now).to_string(),
                    request
                        .schedule_at
                        .map(micros)
                        .unwrap_or_default()
                        .to_string(),
                    replace_mode(request.replace_mode).to_string(),
                ],
            )
            .await?;
        parse_enqueue_response(&request.job_id, response)
    }

    pub(crate) async fn reschedule(
        &self,
        namespace: &str,
        job_id: &str,
        schedule_at: Option<Timestamp>,
    ) -> Result<RescheduleResult> {
        let keys = self.keys(namespace);
        let now = Timestamp::from(std::time::SystemTime::now());
        let response = self
            .eval(
                "reschedule",
                scripts::ENQUEUE_OR_RESCHEDULE,
                &[
                    keys.failures,
                    keys.ready,
                    keys.scheduled,
                    keys.leased,
                    keys.failed,
                    keys.rerun,
                ],
                &[
                    "reschedule".to_string(),
                    job_id.to_string(),
                    micros(now).to_string(),
                    schedule_at.map(micros).unwrap_or_default().to_string(),
                    "replace".to_string(),
                ],
            )
            .await?;
        parse_reschedule_response(job_id, response)
    }

    pub(crate) async fn get_job(&self, namespace: &str, job_id: &str) -> Result<Option<JobRecord>> {
        let keys = self.keys(namespace);
        let mut query = pipe();
        query
            .cmd("ZSCORE")
            .arg(&keys.ready)
            .arg(job_id)
            .cmd("ZSCORE")
            .arg(&keys.leased)
            .arg(job_id)
            .cmd("ZSCORE")
            .arg(&keys.failed)
            .arg(job_id)
            .cmd("ZSCORE")
            .arg(&keys.scheduled)
            .arg(job_id)
            .cmd("HGET")
            .arg(&keys.failures)
            .arg(job_id)
            .cmd("HGET")
            .arg(&keys.lease_meta)
            .arg(job_id);

        let (ready_at, leased_until, failed_at, schedule_at, failure_count, lease_meta): JobLookupFields = self
            .driver
            .query_pipe("get_job", self.config.command_timeout, &query)
            .await?;

        let failure_count = failure_count.unwrap_or_default();
        if let Some(ready_at) = ready_at {
            return Ok(Some(JobRecord {
                job_id: job_id.to_string(),
                state: JobState::Ready,
                ready_at: Some(from_micros(parse_i64("ready_at", &ready_at)?)?),
                schedule_at: None,
                failure_count,
                leased_by: None,
                failed_at: None,
            }));
        }
        if let Some(leased_until) = leased_until {
            let meta = lease_meta.ok_or(Error::InvalidData {
                field: "lease_meta",
            })?;
            let (worker_id, lease_token) = super::codec::parse_lease_meta(&meta)?;
            return Ok(Some(JobRecord {
                job_id: job_id.to_string(),
                state: JobState::Leased,
                ready_at: None,
                schedule_at: None,
                failure_count,
                leased_by: Some(LeaseHolder {
                    worker_id,
                    lease_token,
                    leased_until: from_micros(parse_i64("leased_until", &leased_until)?)?,
                }),
                failed_at: None,
            }));
        }
        if let Some(failed_at) = failed_at {
            return Ok(Some(JobRecord {
                job_id: job_id.to_string(),
                state: JobState::Failed,
                ready_at: None,
                schedule_at: None,
                failure_count,
                leased_by: None,
                failed_at: Some(from_micros(parse_i64("failed_at", &failed_at)?)?),
            }));
        }
        if let Some(schedule_at) = schedule_at {
            return Ok(Some(JobRecord {
                job_id: job_id.to_string(),
                state: JobState::Scheduled,
                ready_at: None,
                schedule_at: Some(from_micros(parse_i64("schedule_at", &schedule_at)?)?),
                failure_count,
                leased_by: None,
                failed_at: None,
            }));
        }
        Ok(None)
    }
}

fn parse_enqueue_response(job_id: &str, response: Vec<String>) -> Result<EnqueueResult> {
    if response.first().map(String::as_str) != Some("ok") {
        return Err(parse_script_error(job_id, &response));
    }
    let status = parse_enqueue_status(response.get(1).ok_or(Error::InvalidData {
        field: "enqueue.status",
    })?)?;
    let state = parse_state(response.get(2).ok_or(Error::InvalidData {
        field: "enqueue.state",
    })?)?;

    Ok(EnqueueResult {
        job_id: job_id.to_string(),
        state,
        status,
    })
}

fn parse_reschedule_response(job_id: &str, response: Vec<String>) -> Result<RescheduleResult> {
    if response.first().map(String::as_str) != Some("ok") {
        return Err(parse_script_error(job_id, &response));
    }
    let state = parse_state(response.get(2).ok_or(Error::InvalidData {
        field: "reschedule.state",
    })?)?;
    let schedule_at = match response.get(3).map(String::as_str) {
        Some("0") | None => None,
        Some(value) => Some(from_micros(parse_i64("reschedule.schedule_at", value)?)?),
    };

    Ok(RescheduleResult {
        job_id: job_id.to_string(),
        state,
        schedule_at,
    })
}
