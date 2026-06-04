//! Redis store construction and `QueueStore` delegation.

use async_trait::async_trait;
use redis::cmd;

use crate::config::{ConnectConfig, Timestamp, WorkerConfig};
use crate::error::Result;
use crate::store::{EnqueueRequest, FailureRequest, QueueStore, ReserveBatch};
use crate::types::{
    EnqueueResult, FailedJobsPage, FailedJobsQuery, FailedSelector, ForceFailRequest, JobRecord,
    QueueCounts, RescheduleResult, WorkerInfo,
};

use super::connection::RedisDriver;
use super::keys::{NamespaceKeys, namespace_keys};

pub(crate) struct RedisStore {
    pub(crate) config: ConnectConfig,
    pub(crate) driver: RedisDriver,
}

impl RedisStore {
    pub(crate) async fn connect(config: ConnectConfig) -> Result<Self> {
        let driver = RedisDriver::connect(&config).await?;
        Ok(Self { config, driver })
    }

    pub(crate) fn keys(&self, namespace: &str) -> NamespaceKeys {
        namespace_keys(&self.config.key_prefix, namespace)
    }

    pub(crate) async fn eval(
        &self,
        operation: &'static str,
        script: &str,
        keys: &[String],
        args: &[String],
    ) -> Result<Vec<String>> {
        let mut command = cmd("EVAL");
        command.arg(script).arg(keys.len());
        for key in keys {
            command.arg(key);
        }
        for arg in args {
            command.arg(arg);
        }
        self.driver
            .query_cmd(operation, self.config.command_timeout, &command)
            .await
    }
}

#[async_trait]
impl QueueStore for RedisStore {
    async fn remember_namespace(&self, namespace: &str) -> Result<()> {
        RedisStore::remember_namespace(self, namespace).await
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        RedisStore::list_namespaces(self).await
    }

    async fn enqueue(&self, namespace: &str, request: EnqueueRequest) -> Result<EnqueueResult> {
        RedisStore::enqueue(self, namespace, request).await
    }

    async fn reschedule(
        &self,
        namespace: &str,
        job_id: &str,
        schedule_at: Option<Timestamp>,
    ) -> Result<RescheduleResult> {
        RedisStore::reschedule(self, namespace, job_id, schedule_at).await
    }

    async fn get_job(&self, namespace: &str, job_id: &str) -> Result<Option<JobRecord>> {
        RedisStore::get_job(self, namespace, job_id).await
    }

    async fn counts(&self, namespace: &str, now: Timestamp) -> Result<QueueCounts> {
        RedisStore::counts(self, namespace, now).await
    }

    async fn list_failed(&self, namespace: &str, query: FailedJobsQuery) -> Result<FailedJobsPage> {
        RedisStore::list_failed(self, namespace, query).await
    }

    async fn list_workers(&self, namespace: &str, now: Timestamp) -> Result<Vec<WorkerInfo>> {
        RedisStore::list_workers(self, namespace, now).await
    }

    async fn cancel(&self, namespace: &str, job_id: &str) -> Result<()> {
        RedisStore::cancel(self, namespace, job_id).await
    }

    async fn force_ack(&self, namespace: &str, job_id: &str) -> Result<()> {
        RedisStore::force_ack(self, namespace, job_id).await
    }

    async fn force_fail(
        &self,
        namespace: &str,
        request: ForceFailRequest,
        now: Timestamp,
    ) -> Result<()> {
        RedisStore::force_fail(self, namespace, request, now).await
    }

    async fn requeue(&self, namespace: &str, job_id: &str, now: Timestamp) -> Result<()> {
        RedisStore::requeue(self, namespace, job_id, now).await
    }

    async fn retry_now(&self, namespace: &str, job_id: &str, now: Timestamp) -> Result<()> {
        RedisStore::retry_now(self, namespace, job_id, now).await
    }

    async fn force_retry(&self, namespace: &str, job_id: &str, now: Timestamp) -> Result<()> {
        RedisStore::force_retry(self, namespace, job_id, now).await
    }

    async fn purge_failed(&self, namespace: &str, selector: FailedSelector) -> Result<u64> {
        RedisStore::purge_failed(self, namespace, selector).await
    }

    async fn reserve(
        &self,
        namespace: &str,
        config: &WorkerConfig,
        available_capacity: usize,
        now: Timestamp,
        lease_tokens: Vec<String>,
    ) -> Result<ReserveBatch> {
        RedisStore::reserve(
            self,
            namespace,
            config,
            available_capacity,
            now,
            lease_tokens,
        )
        .await
    }

    async fn heartbeat(
        &self,
        namespace: &str,
        worker_id: &str,
        job_id: &str,
        lease_token: &str,
        now: Timestamp,
        lease_duration: std::time::Duration,
    ) -> Result<()> {
        RedisStore::heartbeat(
            self,
            namespace,
            worker_id,
            job_id,
            lease_token,
            now,
            lease_duration,
        )
        .await
    }

    async fn ack(
        &self,
        namespace: &str,
        worker_id: &str,
        job_id: &str,
        lease_token: &str,
    ) -> Result<()> {
        RedisStore::ack(self, namespace, worker_id, job_id, lease_token).await
    }

    async fn complete_and_reschedule(
        &self,
        namespace: &str,
        worker_id: &str,
        job_id: &str,
        lease_token: &str,
        schedule_at: Option<Timestamp>,
        now: Timestamp,
    ) -> Result<()> {
        RedisStore::complete_and_reschedule(
            self,
            namespace,
            worker_id,
            job_id,
            lease_token,
            schedule_at,
            now,
        )
        .await
    }

    async fn fail_or_retry(&self, namespace: &str, request: FailureRequest) -> Result<u32> {
        RedisStore::fail_or_retry(self, namespace, request).await
    }

    async fn reap_expired(&self, namespace: &str, now: Timestamp, limit: usize) -> Result<u64> {
        RedisStore::reap_expired(self, namespace, now, limit).await
    }
}
