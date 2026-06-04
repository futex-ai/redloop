use std::time::Duration;

use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use redloop::{ConnectConfig, FailedSelector, RedisDeployment};

use crate::{error::Result, upgrade};

#[derive(Parser)]
#[command(name = "redloop-cli")]
pub(crate) struct Cli {
    #[command(flatten)]
    pub(crate) connect: ConnectArgs,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Args)]
pub(crate) struct ConnectArgs {
    #[arg(long, default_value = "redloop")]
    key_prefix: String,
    #[arg(long, default_value_t = 5_000)]
    command_timeout_ms: u64,
    #[arg(long)]
    redis_url: Option<String>,
    #[arg(long)]
    sentinel_service_name: Option<String>,
    #[arg(long)]
    sentinel_node: Vec<String>,
    #[arg(long)]
    cluster_node: Vec<String>,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    Namespaces,
    Counts(NamespaceArgs),
    GetJob(JobArgs),
    ListFailed(ListFailedArgs),
    ListWorkers(NamespaceArgs),
    Cancel(JobArgs),
    ForceAck(JobArgs),
    ForceFail(ForceFailArgs),
    Requeue(JobArgs),
    RetryNow(JobArgs),
    ForceRetry(JobArgs),
    PurgeFailed(PurgeFailedArgs),
    #[command(alias = "update")]
    Upgrade(upgrade::UpgradeArgs),
}

#[derive(Args)]
pub(crate) struct NamespaceArgs {
    #[arg(long)]
    pub(crate) namespace: String,
}

#[derive(Args)]
pub(crate) struct JobArgs {
    #[arg(long)]
    pub(crate) namespace: String,
    #[arg(long)]
    pub(crate) job_id: String,
}

#[derive(Args)]
pub(crate) struct ForceFailArgs {
    #[arg(long)]
    pub(crate) namespace: String,
    #[arg(long)]
    pub(crate) job_id: String,
    #[arg(long)]
    pub(crate) message: String,
}

#[derive(Args)]
pub(crate) struct ListFailedArgs {
    #[arg(long)]
    pub(crate) namespace: String,
    #[arg(long)]
    pub(crate) cursor: Option<String>,
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,
}

#[derive(Args)]
pub(crate) struct PurgeFailedArgs {
    #[arg(long)]
    pub(crate) namespace: String,
    #[arg(long)]
    job_id: Option<String>,
    #[arg(long)]
    older_than: Option<String>,
    #[arg(long, default_value_t = false)]
    all: bool,
}

impl ConnectArgs {
    pub(crate) fn to_config(&self) -> Result<ConnectConfig> {
        let deployment = if !self.cluster_node.is_empty() {
            RedisDeployment::Cluster {
                nodes: self.cluster_node.clone(),
            }
        } else if !self.sentinel_node.is_empty() || self.sentinel_service_name.is_some() {
            RedisDeployment::Sentinel {
                service_name: self.sentinel_service_name.clone().unwrap_or_default(),
                nodes: self.sentinel_node.clone(),
            }
        } else {
            RedisDeployment::Standalone {
                url: self
                    .redis_url
                    .clone()
                    .unwrap_or_else(|| "redis://127.0.0.1/".to_string()),
            }
        };

        Ok(ConnectConfig {
            deployment,
            key_prefix: self.key_prefix.clone(),
            command_timeout: Duration::from_millis(self.command_timeout_ms),
        })
    }
}

impl PurgeFailedArgs {
    pub(crate) fn selector(&self) -> Result<FailedSelector> {
        if let Some(job_id) = &self.job_id {
            return Ok(FailedSelector::JobId(job_id.clone()));
        }
        if let Some(older_than) = &self.older_than {
            let timestamp = DateTime::parse_from_rfc3339(older_than)
                .map_err(|_| redloop::Error::InvalidCursor {
                    cursor: older_than.clone(),
                })?
                .with_timezone(&Utc);
            return Ok(FailedSelector::OlderThan(timestamp));
        }
        if self.all {
            return Ok(FailedSelector::All);
        }

        Ok(FailedSelector::All)
    }
}
