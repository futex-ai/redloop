use super::{RetryPolicy, WorkerConfig};
use std::time::Duration;

#[test]
fn worker_config_validation_rejects_invalid_intervals() {
    let config = WorkerConfig {
        worker_id: "worker-a".into(),
        concurrency: 1,
        retry_policy: RetryPolicy::Never,
        lease_duration: Duration::from_secs(10),
        heartbeat_interval: Duration::from_secs(10),
        reap_interval: Duration::from_secs(1),
        poll_interval_min: Duration::from_millis(10),
        poll_interval_max: Duration::from_millis(100),
    };

    assert!(config.validate().is_err());
}
