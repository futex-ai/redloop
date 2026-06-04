use redloop::{ConnectConfig, RedisDeployment, RedisRedloopClient, Result};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use testcontainers_modules::{
    redis::{REDIS_PORT, Redis},
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};

pub struct RedisTestContext {
    pub redloop: RedisRedloopClient,
    pub redis_url: String,
    pub key_prefix: String,
    _guard: RedisGuard,
}

enum RedisGuard {
    Docker {
        _container: Box<ContainerAsync<Redis>>,
    },
    Local {
        child: Child,
        dir: PathBuf,
    },
}

impl RedisTestContext {
    pub async fn new() -> Result<Self> {
        let (redis_url, guard) = match Redis::default().start().await {
            Ok(container) => {
                let host = container
                    .get_host()
                    .await
                    .map_err(|_| redloop::Error::InvalidData {
                        field: "docker_host",
                    })?;
                let port = container
                    .get_host_port_ipv4(REDIS_PORT)
                    .await
                    .map_err(|_| redloop::Error::InvalidData {
                        field: "docker_port",
                    })?;
                (
                    format!("redis://{host}:{port}/"),
                    RedisGuard::Docker {
                        _container: Box::new(container),
                    },
                )
            }
            Err(_) => start_local_redis().await?,
        };
        let key_prefix = format!(
            "redloop-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let redloop = RedisRedloopClient::connect(ConnectConfig {
            deployment: RedisDeployment::Standalone {
                url: redis_url.clone(),
            },
            key_prefix: key_prefix.clone(),
            command_timeout: Duration::from_secs(5),
        })
        .await?;

        Ok(Self {
            redloop,
            redis_url,
            key_prefix,
            _guard: guard,
        })
    }

    pub fn namespace_name(&self, label: &str) -> String {
        format!("{label}-{}", chrono::Utc::now().timestamp_micros())
    }
}

impl Drop for RedisGuard {
    fn drop(&mut self) {
        match self {
            RedisGuard::Docker { .. } => {}
            RedisGuard::Local { child, dir } => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }
}

pub async fn wait_until<F, Fut>(timeout: Duration, mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if condition().await {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "condition was not met within {:?}",
            timeout
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn start_local_redis() -> Result<(String, RedisGuard)> {
    let port = reserve_port()?;
    let dir = std::env::temp_dir().join(format!(
        "redloop-redis-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).map_err(|_| redloop::Error::InvalidData {
        field: "local_redis_dir",
    })?;
    let child = Command::new("redis-server")
        .arg("--port")
        .arg(port.to_string())
        .arg("--save")
        .arg("")
        .arg("--appendonly")
        .arg("no")
        .arg("--dir")
        .arg(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| redloop::Error::InvalidData {
            field: "local_redis_spawn",
        })?;
    let redis_url = format!("redis://127.0.0.1:{port}/");
    wait_for_redis(&redis_url).await?;
    Ok((redis_url, RedisGuard::Local { child, dir }))
}

fn reserve_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| redloop::Error::InvalidData {
        field: "local_redis_port_bind",
    })?;
    let port = listener
        .local_addr()
        .map_err(|_| redloop::Error::InvalidData {
            field: "local_redis_port_addr",
        })?
        .port();
    drop(listener);
    Ok(port)
}

async fn wait_for_redis(redis_url: &str) -> Result<()> {
    let client = redis::Client::open(redis_url).map_err(|source| redloop::Error::Redis {
        operation: "test_client_open",
        source,
    })?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(redloop::Error::InvalidData {
                field: "local_redis_ready_timeout",
            });
        }

        if let Ok(mut connection) = client.get_multiplexed_async_connection().await {
            let ping: redis::RedisResult<String> =
                redis::cmd("PING").query_async(&mut connection).await;
            if matches!(ping.as_deref(), Ok("PONG")) {
                return Ok(());
            }
        }

        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
