use chrono::{Duration as ChronoDuration, Utc};
use redloop::{
    Backoff, ConnectConfig, DynRedloopNamespace, JobHandler, JobOutcome, RedisDeployment,
    RedisRedloopClient, RetryPolicy, WorkerConfig,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct AppState {
    processed: AtomicUsize,
}

struct ExampleJobHandler {
    state: Arc<AppState>,
}

#[async_trait::async_trait]
impl JobHandler for ExampleJobHandler {
    async fn handle(&self, job_id: String) -> redloop::HandlerResult {
        let seen = self.state.processed.fetch_add(1, Ordering::Relaxed) + 1;
        println!("processing {job_id} (processed={seen})");
        if job_id.starts_with("daily-digest:") {
            Ok(JobOutcome::Reschedule {
                schedule_at: Some(Utc::now() + ChronoDuration::days(1)),
            })
        } else {
            Ok(JobOutcome::Complete)
        }
    }
}

#[tokio::main]
async fn main() -> redloop::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    let example = Example::from_env().await?;

    match command {
        "enqueue" => {
            let Some(job_id) = args.get(1) else {
                print_usage();
                return Ok(());
            };
            let result = example.queue.job(job_id).execute().await?;
            println!("enqueued {job_id}: {:?}", result.status);
        }
        "schedule" => {
            let Some(job_id) = args.get(1) else {
                print_usage();
                return Ok(());
            };
            let delay_seconds = args
                .get(2)
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(60);
            let schedule_at = Utc::now() + ChronoDuration::seconds(delay_seconds);
            let result = example
                .queue
                .job(job_id)
                .schedule_at(schedule_at)
                .replace_if_earlier()
                .execute()
                .await?;
            println!(
                "scheduled {job_id} for {}: {:?}",
                schedule_at.to_rfc3339(),
                result.status
            );
        }
        "counts" => {
            let counts = example.queue.counts().await?;
            println!(
                "ready={} scheduled_due={} scheduled_future={} leased={} failed={}",
                counts.ready_count,
                counts.scheduled_due_count,
                counts.scheduled_future_count,
                counts.leased_count,
                counts.failed_count
            );
        }
        "work" => {
            let run_for_seconds = args
                .get(1)
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(10);
            example
                .run_worker_for(Duration::from_secs(run_for_seconds))
                .await?;
        }
        _ => print_usage(),
    }

    Ok(())
}

struct Example {
    queue: DynRedloopNamespace,
}

impl Example {
    async fn from_env() -> redloop::Result<Self> {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
        let key_prefix =
            std::env::var("REDLOOP_KEY_PREFIX").unwrap_or_else(|_| "redloop-example".to_string());
        let namespace =
            std::env::var("REDLOOP_NAMESPACE").unwrap_or_else(|_| "example".to_string());

        let redloop = RedisRedloopClient::connect(ConnectConfig {
            deployment: RedisDeployment::Standalone { url: redis_url },
            key_prefix,
            command_timeout: Duration::from_secs(5),
        })
        .await?;

        Ok(Self {
            queue: redloop.namespace(namespace),
        })
    }

    async fn run_worker_for(&self, run_for: Duration) -> redloop::Result<()> {
        let state = Arc::new(AppState {
            processed: AtomicUsize::new(0),
        });
        let worker = self.queue.worker(WorkerConfig {
            worker_id: "example-worker".to_string(),
            concurrency: 4,
            retry_policy: RetryPolicy::Count {
                max_retries: 3,
                backoff: Backoff::Fixed { delay_ms: 1_000 },
            },
            lease_duration: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(10),
            reap_interval: Duration::from_secs(5),
            poll_interval_min: Duration::from_millis(25),
            poll_interval_max: Duration::from_millis(250),
        });

        let result = tokio::time::timeout(
            run_for,
            worker.run(Arc::new(ExampleJobHandler {
                state: state.clone(),
            })),
        )
        .await;

        match result {
            Ok(result) => result?,
            Err(_) => {
                println!("worker stopped after {}s", run_for.as_secs());
            }
        }

        println!("processed {} jobs", state.processed.load(Ordering::Relaxed));
        Ok(())
    }
}

fn print_usage() {
    eprintln!(
        "usage:
  cargo run -p redloop --example basic -- enqueue <job_id>
  cargo run -p redloop --example basic -- schedule <job_id> [delay_seconds]
  cargo run -p redloop --example basic -- counts
  cargo run -p redloop --example basic -- work [seconds]

env:
  REDIS_URL=redis://127.0.0.1/
  REDLOOP_KEY_PREFIX=redloop-example
  REDLOOP_NAMESPACE=example"
    );
}
