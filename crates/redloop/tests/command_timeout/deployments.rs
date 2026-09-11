//! Isolated local Redis processes for Sentinel and single-node Cluster coverage.

use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use redis::aio::MultiplexedConnection;
use tokio::time::{Instant, sleep};
use uuid::Uuid;

/// Owns one Redis process and its temporary configuration/state directory.
pub(super) struct RedisProcess {
    pub(super) port: u16,
    child: Child,
    directory: PathBuf,
}

impl RedisProcess {
    pub(super) async fn standalone() -> Self {
        Self::start("").await
    }

    pub(super) async fn sentinel(master_port: u16) -> Self {
        Self::start(&format!(
            "sentinel monitor test-master 127.0.0.1 {master_port} 1\nsentinel down-after-milliseconds test-master 10000\n"
        ))
        .await
    }

    pub(super) async fn cluster() -> Self {
        let server = Self::start("cluster-enabled yes\ncluster-config-file nodes.conf\n").await;
        let mut connection = server.connection().await;
        redis::cmd("CLUSTER")
            .arg("ADDSLOTS")
            .arg((0..16384).collect::<Vec<u16>>())
            .query_async::<()>(&mut connection)
            .await
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let info: String = redis::cmd("CLUSTER")
                .arg("INFO")
                .query_async(&mut connection)
                .await
                .unwrap();
            if info.lines().any(|line| line == "cluster_state:ok") {
                return server;
            }
            assert!(
                Instant::now() < deadline,
                "cluster never became ready: {info}"
            );
            sleep(Duration::from_millis(25)).await;
        }
    }

    async fn start(extra: &str) -> Self {
        let port = loop {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            if port <= 55000 && TcpListener::bind(("127.0.0.1", port + 10000)).is_ok() {
                break port;
            }
        };
        let directory = std::env::temp_dir().join(format!("redloop-topology-{}", Uuid::now_v7()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("redis.conf");
        fs::write(
            &path,
            format!(
                "bind 127.0.0.1\nport {port}\nsave \"\"\nappendonly no\ndir {}\n{extra}",
                directory.display()
            ),
        )
        .unwrap();
        let mut command = Command::new("redis-server");
        command
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if extra.starts_with("sentinel ") {
            command.arg("--sentinel");
        }
        let child = command
            .spawn()
            .expect("command_timeout tests require redis-server on PATH");
        let mut server = Self {
            port,
            child,
            directory,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                server.child.try_wait().unwrap().is_none(),
                "Redis exited during startup"
            );
            if server
                .client()
                .get_multiplexed_async_connection()
                .await
                .is_ok()
            {
                return server;
            }
            assert!(Instant::now() < deadline, "Redis startup timed out");
            sleep(Duration::from_millis(25)).await;
        }
    }

    pub(super) fn url(&self) -> String {
        format!("redis://127.0.0.1:{}/", self.port)
    }

    fn client(&self) -> redis::Client {
        redis::Client::open(self.url()).unwrap()
    }

    pub(super) async fn connection(&self) -> MultiplexedConnection {
        self.client()
            .get_multiplexed_async_connection()
            .await
            .unwrap()
    }

    pub(super) async fn pause(&self, duration: Duration) {
        redis::cmd("CLIENT")
            .arg("PAUSE")
            .arg(duration.as_millis() as u64)
            .arg("ALL")
            .query_async::<()>(&mut self.connection().await)
            .await
            .unwrap();
    }
}

impl Drop for RedisProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.directory);
    }
}
