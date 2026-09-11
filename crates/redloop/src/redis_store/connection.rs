//! Redis connections with one response policy and bounded command execution.

use std::{result, sync::Arc, time::Duration};

use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::cluster::ClusterClientBuilder;
use redis::cluster_async::ClusterConnection;
use redis::sentinel::{SentinelClient, SentinelServerType};
use redis::{Client, Cmd, FromRedisValue, Pipeline, RedisResult};
use tokio::sync::Mutex;
use tokio::time::{error::Elapsed, timeout};

use crate::config::{ConnectConfig, RedisDeployment};
use crate::error::{Error, Result};

pub(crate) enum RedisDriver {
    Single {
        manager: ConnectionManager,
    },
    Cluster {
        connection: Arc<Mutex<ClusterConnection>>,
    },
}

impl RedisDriver {
    pub(crate) async fn connect(config: &ConnectConfig) -> Result<Self> {
        let response_config =
            ConnectionManagerConfig::new().set_response_timeout(Some(config.command_timeout));
        match &config.deployment {
            RedisDeployment::Standalone { url } => {
                let client = redis_result("client_open", Client::open(url.clone()))?;
                let manager = redis_result(
                    "connection_manager_new",
                    ConnectionManager::new_with_config(client, response_config).await,
                )?;
                Ok(Self::Single { manager })
            }
            RedisDeployment::Sentinel {
                service_name,
                nodes,
            } => {
                let mut sentinel = redis_result(
                    "sentinel_build",
                    SentinelClient::build(
                        nodes.clone(),
                        service_name.clone(),
                        None,
                        SentinelServerType::Master,
                    ),
                )?;
                let client = redis_result("sentinel_get_client", sentinel.get_client())?;
                let manager = redis_result(
                    "connection_manager_new",
                    ConnectionManager::new_with_config(client, response_config).await,
                )?;
                Ok(Self::Single { manager })
            }
            RedisDeployment::Cluster { nodes } => {
                let client = redis_result(
                    "cluster_build",
                    ClusterClientBuilder::new(nodes.clone())
                        .response_timeout(config.command_timeout)
                        .overall_response_timeout(Some(config.command_timeout))
                        .build(),
                )?;
                let connection = redis_result(
                    "cluster_get_async_connection",
                    client.get_async_connection().await,
                )?;
                Ok(Self::Cluster {
                    connection: Arc::new(Mutex::new(connection)),
                })
            }
        }
    }

    pub(crate) async fn query_cmd<T: FromRedisValue>(
        &self,
        operation: &'static str,
        command_timeout: Duration,
        cmd: &Cmd,
    ) -> Result<T> {
        let response = timeout(command_timeout, async {
            match self {
                Self::Single { manager } => {
                    let mut connection = manager.clone();
                    cmd.query_async(&mut connection).await
                }
                Self::Cluster { connection } => {
                    let mut connection = connection.lock().await;
                    cmd.query_async(&mut *connection).await
                }
            }
        })
        .await;
        command_result(operation, command_timeout, response)
    }

    pub(crate) async fn query_pipe<T: FromRedisValue>(
        &self,
        operation: &'static str,
        command_timeout: Duration,
        pipe: &Pipeline,
    ) -> Result<T> {
        let response = timeout(command_timeout, async {
            match self {
                Self::Single { manager } => {
                    let mut connection = manager.clone();
                    pipe.query_async(&mut connection).await
                }
                Self::Cluster { connection } => {
                    let mut connection = connection.lock().await;
                    pipe.query_async(&mut *connection).await
                }
            }
        })
        .await;
        command_result(operation, command_timeout, response)
    }
}

fn redis_result<T>(operation: &'static str, response: RedisResult<T>) -> Result<T> {
    match response {
        Ok(value) => Ok(value),
        Err(source) => Err(Error::Redis { operation, source }),
    }
}

/// Normalizes either timer winning the race without discarding other Redis errors.
fn command_result<T>(
    operation: &'static str,
    command_timeout: Duration,
    response: result::Result<RedisResult<T>, Elapsed>,
) -> Result<T> {
    match response {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(source)) if !source.is_timeout() => Err(Error::Redis { operation, source }),
        _ => Err(Error::CommandTimedOut {
            operation,
            timeout_ms: command_timeout.as_millis().min(u64::MAX as u128) as u64,
        }),
    }
}

#[cfg(test)]
#[path = "_tests_/connection_tests.rs"]
mod connection_tests;
