use crate::config::WorkerConfig;
use crate::error::{Error, Result};

pub(crate) struct StoredWorkerConfig {
    pub concurrency: usize,
    pub lease_duration_ms: u64,
}

pub(crate) fn pack_worker_config_record(config: &WorkerConfig) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}",
        config.concurrency,
        config.lease_duration.as_millis(),
        config.heartbeat_interval.as_millis(),
        config.reap_interval.as_millis(),
        config.poll_interval_min.as_millis(),
        config.poll_interval_max.as_millis(),
        config.retry_policy_key()
    )
}

pub(crate) fn parse_worker_config_record(record: &str) -> Result<StoredWorkerConfig> {
    let mut parts = record.split(':');
    let concurrency = parts
        .next()
        .ok_or(Error::InvalidData {
            field: "workers:config.concurrency",
        })?
        .parse::<usize>()
        .map_err(|_| Error::InvalidData {
            field: "workers:config.concurrency",
        })?;
    let lease_duration_ms = parts
        .next()
        .ok_or(Error::InvalidData {
            field: "workers:config.lease_duration_ms",
        })?
        .parse::<u64>()
        .map_err(|_| Error::InvalidData {
            field: "workers:config.lease_duration_ms",
        })?;

    Ok(StoredWorkerConfig {
        concurrency,
        lease_duration_ms,
    })
}

pub(crate) fn parse_lease_meta(meta: &str) -> Result<(String, String)> {
    let delimiter = meta.find(':').ok_or(Error::InvalidData {
        field: "lease_meta.delimiter",
    })?;
    let length = meta[..delimiter]
        .parse::<usize>()
        .map_err(|_| Error::InvalidData {
            field: "lease_meta.length",
        })?;
    let rest = &meta[delimiter + 1..];
    if rest.len() < length {
        return Err(Error::InvalidData {
            field: "lease_meta.worker_id",
        });
    }

    let worker_id = rest[..length].to_string();
    let lease_token = rest[length..].to_string();
    Ok((worker_id, lease_token))
}
