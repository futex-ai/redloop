use crate::config::Timestamp;
use async_trait::async_trait;
use chrono::Utc;
use std::time::Duration;

#[cfg_attr(test, unimock::unimock(api = WorkerCoordinatorMock))]
#[async_trait]
pub(crate) trait WorkerCoordinator: Send + Sync {
    async fn sleep(&self, duration: Duration);
}

#[cfg_attr(test, unimock::unimock(api = ClockMock))]
pub(crate) trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

pub(crate) struct SystemClock;

pub(crate) struct TokioWorkerCoordinator;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Utc::now()
    }
}

#[async_trait]
impl WorkerCoordinator for TokioWorkerCoordinator {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}
