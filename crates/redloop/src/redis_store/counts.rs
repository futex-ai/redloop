//! Queue counts and failed-job list operations.

use redis::{cmd, pipe};

use crate::config::Timestamp;
use crate::error::{Error, Result};
use crate::types::{
    FailedJobsPage, FailedJobsQuery, FailedSelector, JobRecord, JobState, QueueCounts,
};

use super::RedisStore;
use super::shared::{from_micros, hmget, micros};

impl RedisStore {
    pub(crate) async fn counts(&self, namespace: &str, now: Timestamp) -> Result<QueueCounts> {
        let keys = self.keys(namespace);
        let mut query = pipe();
        query
            .cmd("ZCARD")
            .arg(&keys.ready)
            .cmd("ZCOUNT")
            .arg(&keys.scheduled)
            .arg("-inf")
            .arg(micros(now))
            .cmd("ZCOUNT")
            .arg(&keys.scheduled)
            .arg(format!("({}", micros(now)))
            .arg("+inf")
            .cmd("ZCARD")
            .arg(&keys.leased)
            .cmd("ZCARD")
            .arg(&keys.failed);
        let (ready_count, scheduled_due_count, scheduled_future_count, leased_count, failed_count): (
            u64,
            u64,
            u64,
            u64,
            u64,
        ) = self
            .driver
            .query_pipe("counts", self.config.command_timeout, &query)
            .await?;
        Ok(QueueCounts {
            ready_count,
            scheduled_due_count,
            scheduled_future_count,
            leased_count,
            failed_count,
        })
    }

    pub(crate) async fn list_failed(
        &self,
        namespace: &str,
        query: FailedJobsQuery,
    ) -> Result<FailedJobsPage> {
        let offset = query
            .cursor
            .as_deref()
            .map(parse_cursor)
            .transpose()?
            .unwrap_or_default();
        if query.limit == 0 {
            return Ok(FailedJobsPage {
                jobs: Vec::new(),
                next_cursor: None,
            });
        }

        let keys = self.keys(namespace);
        let mut failed_query = cmd("ZRANGE");
        failed_query
            .arg(&keys.failed)
            .arg(offset)
            .arg(offset + query.limit)
            .arg("WITHSCORES");
        let mut failed_rows: Vec<(String, i64)> = self
            .driver
            .query_cmd("list_failed", self.config.command_timeout, &failed_query)
            .await?;

        let next_cursor = if failed_rows.len() > query.limit {
            failed_rows.truncate(query.limit);
            Some((offset + query.limit).to_string())
        } else {
            None
        };

        let job_ids: Vec<String> = failed_rows
            .iter()
            .map(|(job_id, _)| job_id.clone())
            .collect();
        let failure_counts: Vec<Option<u32>> = hmget(
            &self.driver,
            self.config.command_timeout,
            "failed_hmget",
            &keys.failures,
            &job_ids,
        )
        .await?;

        let jobs = failed_rows
            .into_iter()
            .zip(failure_counts)
            .map(|((job_id, failed_at), failure_count)| JobRecord {
                job_id,
                state: JobState::Failed,
                ready_at: None,
                schedule_at: None,
                failure_count: failure_count.unwrap_or_default(),
                leased_by: None,
                failed_at: Some(
                    from_micros(failed_at)
                        .unwrap_or_else(|_| Timestamp::from(std::time::SystemTime::UNIX_EPOCH)),
                ),
            })
            .collect();

        Ok(FailedJobsPage { jobs, next_cursor })
    }

    pub(crate) async fn purge_failed(
        &self,
        namespace: &str,
        selector: FailedSelector,
    ) -> Result<u64> {
        let keys = self.keys(namespace);
        let job_ids = match selector {
            FailedSelector::JobId(job_id) => {
                let mut check = cmd("ZSCORE");
                check.arg(&keys.failed).arg(&job_id);
                let exists: Option<String> = self
                    .driver
                    .query_cmd("purge_failed_check", self.config.command_timeout, &check)
                    .await?;
                if exists.is_some() {
                    vec![job_id]
                } else {
                    Vec::new()
                }
            }
            FailedSelector::OlderThan(timestamp) => {
                let mut query = cmd("ZRANGEBYSCORE");
                query
                    .arg(&keys.failed)
                    .arg("-inf")
                    .arg(format!("({}", super::shared::micros(timestamp)));
                self.driver
                    .query_cmd("purge_failed_range", self.config.command_timeout, &query)
                    .await?
            }
            FailedSelector::All => {
                let mut query = cmd("ZRANGE");
                query.arg(&keys.failed).arg(0).arg(-1);
                self.driver
                    .query_cmd("purge_failed_all", self.config.command_timeout, &query)
                    .await?
            }
        };

        if job_ids.is_empty() {
            return Ok(0);
        }

        let mut cleanup = pipe();
        cleanup.atomic();
        cleanup.cmd("ZREM").arg(&keys.failed).arg(&job_ids);
        cleanup.cmd("HDEL").arg(&keys.failures).arg(&job_ids);
        let _: (u64, u64) = self
            .driver
            .query_pipe("purge_failed", self.config.command_timeout, &cleanup)
            .await?;
        Ok(job_ids.len() as u64)
    }
}

pub(crate) fn parse_cursor(cursor: &str) -> Result<usize> {
    cursor.parse::<usize>().map_err(|_| Error::InvalidCursor {
        cursor: cursor.to_string(),
    })
}
