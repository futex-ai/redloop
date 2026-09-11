//! Command error classification is independent of which timeout fires first.

use std::{io, time::Duration};

use redis::{ErrorKind, RedisError};

use crate::error::Error;

use super::command_result;

#[test]
fn redis_response_timeout_uses_the_configured_command_error() {
    let source = io::Error::new(io::ErrorKind::TimedOut, "delayed reply");
    let error = command_result::<()>("reserve", Duration::from_secs(5), Ok(Err(source.into())))
        .unwrap_err();
    assert!(matches!(
        error,
        Error::CommandTimedOut {
            operation: "reserve",
            timeout_ms: 5000
        }
    ));
    assert!(error.is_recoverable_worker_runtime());
}

#[test]
fn other_redis_errors_keep_their_source_and_classification() {
    let source = RedisError::from((ErrorKind::AuthenticationFailed, "invalid credential"));
    let error =
        command_result::<()>("reserve", Duration::from_secs(5), Ok(Err(source))).unwrap_err();
    assert!(
        matches!(&error, Error::Redis { operation: "reserve", source } if source.kind() == ErrorKind::AuthenticationFailed)
    );
    assert!(!error.is_recoverable_worker_runtime());
}
