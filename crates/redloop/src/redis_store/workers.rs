//! Worker listing, reservation, and stale-worker cleanup.

use redis::{cmd, pipe};

use crate::config::{Timestamp, WorkerConfig};
use crate::error::{Error, Result};
use crate::store::{ReserveBatch, ReservedJob};
use crate::types::WorkerInfo;

use super::RedisStore;
use super::codec::{pack_worker_config_record, parse_worker_config_record};
use super::scripts;
use super::shared::{from_micros, hmget, micros, parse_i64, parse_script_error};

impl RedisStore {
    pub(crate) async fn list_workers(
        &self,
        namespace: &str,
        now: Timestamp,
    ) -> Result<Vec<WorkerInfo>> {
        self.prune_stale_workers(namespace, now).await?;

        let keys = self.keys(namespace);
        let mut workers_query = cmd("ZRANGE");
        workers_query
            .arg(&keys.workers_last_seen)
            .arg(0)
            .arg(-1)
            .arg("WITHSCORES");
        let workers: Vec<(String, i64)> = self
            .driver
            .query_cmd("list_workers", self.config.command_timeout, &workers_query)
            .await?;
        if workers.is_empty() {
            return Ok(Vec::new());
        }

        let worker_ids: Vec<String> = workers
            .iter()
            .map(|(worker_id, _)| worker_id.clone())
            .collect();
        let configs: Vec<Option<String>> = hmget(
            &self.driver,
            self.config.command_timeout,
            "workers_config_hmget",
            &keys.workers_config,
            &worker_ids,
        )
        .await?;
        let lease_counts: Vec<Option<u64>> = hmget(
            &self.driver,
            self.config.command_timeout,
            "workers_leases_hmget",
            &keys.workers_leases,
            &worker_ids,
        )
        .await?;

        let mut result = Vec::new();
        for (((worker_id, last_seen_us), config_record), leased_count) in
            workers.into_iter().zip(configs).zip(lease_counts)
        {
            let Some(config_record) = config_record else {
                continue;
            };
            let parsed = parse_worker_config_record(&config_record)?;
            result.push(WorkerInfo {
                worker_id,
                concurrency: parsed.concurrency,
                leased_count: leased_count.unwrap_or_default(),
                last_heartbeat_at: from_micros(last_seen_us)?,
            });
        }

        Ok(result)
    }

    pub(crate) async fn reserve(
        &self,
        namespace: &str,
        config: &WorkerConfig,
        available_capacity: usize,
        now: Timestamp,
        lease_tokens: Vec<String>,
    ) -> Result<ReserveBatch> {
        if available_capacity == 0 {
            return Ok(ReserveBatch {
                jobs: Vec::new(),
                next_schedule_at: None,
            });
        }

        let keys = self.keys(namespace);
        let mut args = vec![
            config.worker_id.clone(),
            micros(now).to_string(),
            available_capacity.to_string(),
            super::shared::duration_micros(config.lease_duration).to_string(),
            config.retry_policy_key(),
            pack_worker_config_record(config),
        ];
        args.extend(lease_tokens);

        let response = self
            .eval(
                "reserve",
                scripts::RESERVE,
                &[
                    keys.cfg,
                    keys.ready,
                    keys.scheduled,
                    keys.leased,
                    keys.lease_meta,
                    keys.workers_last_seen,
                    keys.workers_config,
                    keys.workers_leases,
                ],
                &args,
            )
            .await?;

        if response.first().map(String::as_str) != Some("ok") {
            return Err(parse_script_error(&config.worker_id, &response));
        }

        let next_schedule_at = match response.get(1).map(String::as_str) {
            Some("0") | None => None,
            Some(value) => Some(from_micros(parse_i64("next_schedule_at", value)?)?),
        };

        let mut jobs = Vec::new();
        for chunk in response[2..].chunks(3) {
            if chunk.len() != 3 {
                return Err(Error::InvalidData {
                    field: "reserve.response",
                });
            }
            let _ = parse_i64("leased_until", &chunk[2])?;
            jobs.push(ReservedJob {
                job_id: chunk[0].clone(),
                lease_token: chunk[1].clone(),
            });
        }

        if let Err(error) = self.remember_namespace(namespace).await {
            tracing::warn!(
                namespace,
                ?error,
                "failed to remember namespace after reserve; returning reserved jobs"
            );
        }
        Ok(ReserveBatch {
            jobs,
            next_schedule_at,
        })
    }

    async fn prune_stale_workers(&self, namespace: &str, now: Timestamp) -> Result<()> {
        let keys = self.keys(namespace);
        let mut workers_command = cmd("ZRANGE");
        workers_command
            .arg(&keys.workers_last_seen)
            .arg(0)
            .arg(-1)
            .arg("WITHSCORES");
        let workers: Vec<(String, i64)> = self
            .driver
            .query_cmd(
                "workers_zrange",
                self.config.command_timeout,
                &workers_command,
            )
            .await?;
        if workers.is_empty() {
            return Ok(());
        }

        let worker_ids: Vec<String> = workers
            .iter()
            .map(|(worker_id, _)| worker_id.clone())
            .collect();
        let configs: Vec<Option<String>> = hmget(
            &self.driver,
            self.config.command_timeout,
            "workers_config_hmget",
            &keys.workers_config,
            &worker_ids,
        )
        .await?;

        let now_us = micros(now);
        let mut stale_workers = Vec::new();
        for ((worker_id, last_seen_us), config_record) in workers.iter().zip(configs.iter()) {
            let Some(config_record) = config_record else {
                stale_workers.push(worker_id.clone());
                continue;
            };
            let parsed = parse_worker_config_record(config_record)?;
            let lease_window_us = (parsed.lease_duration_ms as i64)
                .saturating_mul(10)
                .saturating_mul(1_000);
            let stale_after_us = lease_window_us.max(24_i64 * 60 * 60 * 1_000_000);
            if now_us.saturating_sub(*last_seen_us) > stale_after_us {
                stale_workers.push(worker_id.clone());
            }
        }

        if stale_workers.is_empty() {
            return Ok(());
        }

        let mut cleanup = pipe();
        cleanup.atomic();
        for worker_id in &stale_workers {
            cleanup
                .cmd("ZREM")
                .arg(&keys.workers_last_seen)
                .arg(worker_id);
            cleanup.cmd("HDEL").arg(&keys.workers_config).arg(worker_id);
            cleanup.cmd("HDEL").arg(&keys.workers_leases).arg(worker_id);
        }
        let _: () = self
            .driver
            .query_pipe("workers_cleanup", self.config.command_timeout, &cleanup)
            .await?;
        Ok(())
    }
}
