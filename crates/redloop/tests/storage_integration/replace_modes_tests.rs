//! Replace-mode storage integration tests.

use chrono::{Duration as ChronoDuration, Utc};
use redloop::{EnqueueStatus, JobState};

use super::support::{RedisTestContext, persisted_timestamp};

#[tokio::test]
async fn replace_modes_respect_waiting_activation_time() -> redloop::Result<()> {
    let ctx = RedisTestContext::new().await?;
    let queue = ctx.redloop.namespace(ctx.namespace_name("replace"));

    let later = persisted_timestamp(Utc::now() + ChronoDuration::minutes(30));
    let earlier = persisted_timestamp(Utc::now() + ChronoDuration::minutes(5));
    queue.job("job-2").schedule_at(later).execute().await?;

    let unchanged = queue
        .job("job-2")
        .schedule_at(later + ChronoDuration::minutes(10))
        .replace_if_earlier()
        .execute()
        .await?;
    assert_eq!(unchanged.status, EnqueueStatus::Unchanged);
    assert_eq!(unchanged.state, JobState::Scheduled);

    let updated = queue
        .job("job-2")
        .schedule_at(earlier)
        .replace_if_earlier()
        .execute()
        .await?;
    assert_eq!(updated.status, EnqueueStatus::Updated);
    assert_eq!(updated.state, JobState::Scheduled);

    let record = queue.get_job("job-2").await?.expect("job should exist");
    assert_eq!(record.schedule_at, Some(earlier));
    Ok(())
}
