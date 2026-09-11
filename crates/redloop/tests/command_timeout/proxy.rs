//! A loopback proxy that delays complete Redis replies after commands execute.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use redis::Value;
use tokio::io::{AsyncWriteExt, BufReader, copy};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task::{JoinHandle, JoinSet};

struct DelayedReply {
    job_id: Option<String>,
    duration: Duration,
    received: oneshot::Sender<()>,
}

/// Owns the listener and every proxied connection for one integration test.
pub(super) struct ResponseProxy {
    pub(super) url: String,
    next_reply: Arc<Mutex<Option<DelayedReply>>>,
    disconnect: mpsc::Sender<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl ResponseProxy {
    pub(super) async fn new(redis_url: &str) -> Self {
        let backend = redis_url
            .strip_prefix("redis://")
            .expect("test Redis URL")
            .trim_end_matches('/')
            .to_owned();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("redis://{}/", listener.local_addr().unwrap());
        let next_reply = Arc::new(Mutex::new(None));
        let delays = Arc::clone(&next_reply);
        let (disconnect, mut disconnects) = mpsc::channel::<oneshot::Sender<()>>(1);
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (client, _) = accepted.unwrap();
                        let backend = backend.clone();
                        let delays = Arc::clone(&delays);
                        connections.spawn(async move {
                            let server = TcpStream::connect(backend).await.unwrap();
                            client.set_nodelay(true).unwrap();
                            server.set_nodelay(true).unwrap();
                            forward(client, server, delays).await;
                        });
                    }
                    Some(done) = disconnects.recv() => {
                        connections.abort_all();
                        while connections.join_next().await.is_some() {}
                        let _ = done.send(());
                    }
                    Some(result) = connections.join_next(), if !connections.is_empty() => {
                        result.unwrap();
                    }
                }
            }
        });
        Self {
            url,
            next_reply,
            disconnect,
            task,
        }
    }

    pub(super) fn delay_next(&self, duration: Duration) -> oneshot::Receiver<()> {
        self.delay_reply(None, duration)
    }

    pub(super) fn delay_reservation(
        &self,
        job_id: &str,
        duration: Duration,
    ) -> oneshot::Receiver<()> {
        self.delay_reply(Some(job_id.to_owned()), duration)
    }

    fn delay_reply(&self, job_id: Option<String>, duration: Duration) -> oneshot::Receiver<()> {
        let (received, notification) = oneshot::channel();
        let previous = self.next_reply.lock().unwrap().replace(DelayedReply {
            job_id,
            duration,
            received,
        });
        assert!(
            previous.is_none(),
            "only one reply may be delayed at a time"
        );
        notification
    }

    pub(super) async fn disconnect(&self) {
        let (done, disconnected) = oneshot::channel();
        self.disconnect.send(done).await.unwrap();
        disconnected.await.unwrap();
    }
}

impl Drop for ResponseProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn forward(
    mut client: TcpStream,
    mut server: TcpStream,
    delays: Arc<Mutex<Option<DelayedReply>>>,
) {
    let (mut requests, mut responses) = client.split();
    let (server_read, mut server_write) = server.split();
    let mut reader = BufReader::new(server_read);
    let mut decoder = Default::default();
    tokio::select! {
        _ = copy(&mut requests, &mut server_write) => {}
        _ = async {
            while let Ok(reply) = redis::parse_redis_value_async(&mut decoder, &mut reader).await {
                let delayed = {
                    let mut pending = delays.lock().unwrap();
                    if pending.as_ref().is_some_and(|delay| {
                        delay.job_id.as_ref().is_none_or(|job_id| contains(&reply, job_id))
                    }) {
                        pending.take()
                    } else {
                        None
                    }
                };
                if let Some(delayed) = delayed {
                    let _ = delayed.received.send(());
                    tokio::time::sleep(delayed.duration).await;
                }
                let mut encoded = Vec::new();
                encode(&reply, &mut encoded);
                if responses.write_all(&encoded).await.is_err() {
                    return;
                }
            }
        } => {}
    }
}

fn contains(reply: &Value, job_id: &str) -> bool {
    match reply {
        Value::BulkString(bytes) => bytes == job_id.as_bytes(),
        Value::Array(values) => values.iter().any(|value| contains(value, job_id)),
        _ => false,
    }
}

fn encode(reply: &Value, output: &mut Vec<u8>) {
    match reply {
        Value::Nil => output.extend_from_slice(b"$-1\r\n"),
        Value::Okay => output.extend_from_slice(b"+OK\r\n"),
        Value::Int(value) => output.extend_from_slice(format!(":{value}\r\n").as_bytes()),
        Value::SimpleString(value) => {
            output.extend_from_slice(format!("+{value}\r\n").as_bytes());
        }
        Value::BulkString(bytes) => {
            output.extend_from_slice(format!("${}\r\n", bytes.len()).as_bytes());
            output.extend_from_slice(bytes);
            output.extend_from_slice(b"\r\n");
        }
        Value::Array(values) => {
            output.extend_from_slice(format!("*{}\r\n", values.len()).as_bytes());
            for value in values {
                encode(value, output);
            }
        }
        Value::ServerError(error) => {
            output.extend_from_slice(format!("-{}", error.code()).as_bytes());
            if let Some(details) = error.details() {
                output.extend_from_slice(format!(" {details}").as_bytes());
            }
            output.extend_from_slice(b"\r\n");
        }
        value => panic!("unexpected Redis test reply: {value:?}"),
    }
}
