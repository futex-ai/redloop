use crate::config::{ConnectConfig, RedisDeployment};
use crate::error::{Error, Result};
use redis::aio::ConnectionManager;
use redis::cluster::ClusterClientBuilder;
use redis::cluster_async::ClusterConnection;
use redis::sentinel::{SentinelClient, SentinelServerType};
use redis::{Cmd, Pipeline};
use std::sync::Arc;
use tokio::sync::Mutex;

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
        match &config.deployment {
            RedisDeployment::Standalone { url } => {
                let client = redis::Client::open(url.clone()).map_err(|source| Error::Redis {
                    operation: "client_open",
                    source,
                })?;
                let manager =
                    ConnectionManager::new(client)
                        .await
                        .map_err(|source| Error::Redis {
                            operation: "connection_manager_new",
                            source,
                        })?;
                Ok(Self::Single { manager })
            }
            RedisDeployment::Sentinel {
                service_name,
                nodes,
            } => {
                let mut sentinel = SentinelClient::build(
                    nodes.clone(),
                    service_name.clone(),
                    None,
                    SentinelServerType::Master,
                )
                .map_err(|source| Error::Redis {
                    operation: "sentinel_build",
                    source,
                })?;
                let client = sentinel.get_client().map_err(|source| Error::Redis {
                    operation: "sentinel_get_client",
                    source,
                })?;
                let manager =
                    ConnectionManager::new(client)
                        .await
                        .map_err(|source| Error::Redis {
                            operation: "connection_manager_new",
                            source,
                        })?;
                Ok(Self::Single { manager })
            }
            RedisDeployment::Cluster { nodes } => {
                let client =
                    ClusterClientBuilder::new(nodes.clone())
                        .build()
                        .map_err(|source| Error::Redis {
                            operation: "cluster_build",
                            source,
                        })?;
                let connection =
                    client
                        .get_async_connection()
                        .await
                        .map_err(|source| Error::Redis {
                            operation: "cluster_get_async_connection",
                            source,
                        })?;
                Ok(Self::Cluster {
                    connection: Arc::new(Mutex::new(connection)),
                })
            }
        }
    }

    pub(crate) async fn query_cmd<T: redis::FromRedisValue>(
        &self,
        operation: &'static str,
        timeout: std::time::Duration,
        cmd: &Cmd,
    ) -> Result<T> {
        match self {
            RedisDriver::Single { manager } => {
                let mut connection = manager.clone();
                tokio::time::timeout(timeout, cmd.query_async(&mut connection))
                    .await
                    .map_err(|_| Error::CommandTimedOut {
                        operation,
                        timeout_ms: timeout.as_millis() as u64,
                    })?
                    .map_err(|source| Error::Redis { operation, source })
            }
            RedisDriver::Cluster { connection } => {
                let mut connection = connection.lock().await;
                tokio::time::timeout(timeout, cmd.query_async(&mut *connection))
                    .await
                    .map_err(|_| Error::CommandTimedOut {
                        operation,
                        timeout_ms: timeout.as_millis() as u64,
                    })?
                    .map_err(|source| Error::Redis { operation, source })
            }
        }
    }

    pub(crate) async fn query_pipe<T: redis::FromRedisValue>(
        &self,
        operation: &'static str,
        timeout: std::time::Duration,
        pipe: &Pipeline,
    ) -> Result<T> {
        match self {
            RedisDriver::Single { manager } => {
                let mut connection = manager.clone();
                tokio::time::timeout(timeout, pipe.query_async(&mut connection))
                    .await
                    .map_err(|_| Error::CommandTimedOut {
                        operation,
                        timeout_ms: timeout.as_millis() as u64,
                    })?
                    .map_err(|source| Error::Redis { operation, source })
            }
            RedisDriver::Cluster { connection } => {
                let mut connection = connection.lock().await;
                tokio::time::timeout(timeout, pipe.query_async(&mut *connection))
                    .await
                    .map_err(|_| Error::CommandTimedOut {
                        operation,
                        timeout_ms: timeout.as_millis() as u64,
                    })?
                    .map_err(|source| Error::Redis { operation, source })
            }
        }
    }
}
