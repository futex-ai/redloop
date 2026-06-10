//! Error classification tests.

use std::io;

use crate::error::Error;
use crate::types::JobState;

fn redis_io_error(kind: io::ErrorKind) -> Error {
    Error::Redis {
        operation: "test_operation",
        source: io::Error::new(kind, "test redis transport error").into(),
    }
}

#[test]
fn command_timeout_is_recoverable_worker_runtime() {
    let error = Error::CommandTimedOut {
        operation: "reserve",
        timeout_ms: 50,
    };

    assert!(error.is_recoverable_worker_runtime());
}

#[test]
fn redis_timeout_is_recoverable_worker_runtime() {
    assert!(redis_io_error(io::ErrorKind::TimedOut).is_recoverable_worker_runtime());
    assert!(redis_io_error(io::ErrorKind::WouldBlock).is_recoverable_worker_runtime());
}

#[test]
fn redis_connection_refusal_is_recoverable_worker_runtime() {
    assert!(redis_io_error(io::ErrorKind::ConnectionRefused).is_recoverable_worker_runtime());
}

#[test]
fn redis_connection_dropped_is_recoverable_worker_runtime() {
    let dropped_connection_kinds = [
        io::ErrorKind::BrokenPipe,
        io::ErrorKind::ConnectionReset,
        io::ErrorKind::ConnectionAborted,
        io::ErrorKind::UnexpectedEof,
        io::ErrorKind::NotConnected,
    ];

    for kind in dropped_connection_kinds {
        assert!(redis_io_error(kind).is_recoverable_worker_runtime());
    }
}

#[test]
fn non_transport_redis_error_is_not_recoverable_worker_runtime() {
    let error = Error::Redis {
        operation: "reserve",
        source: (redis::ErrorKind::Client, "client error").into(),
    };

    assert!(!error.is_recoverable_worker_runtime());
}

#[test]
fn non_transport_runtime_errors_are_not_recoverable_worker_runtime() {
    let lease_mismatch = Error::LeaseMismatch {
        job_id: "job-1".to_owned(),
    };
    let invalid_state = Error::InvalidState {
        job_id: "job-1".to_owned(),
        state: JobState::Failed,
    };

    assert!(!lease_mismatch.is_recoverable_worker_runtime());
    assert!(!invalid_state.is_recoverable_worker_runtime());
}
