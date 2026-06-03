use redloop::{ForceFailRequest, RedisRedloopClient};

use crate::{
    cli::{ForceFailArgs, JobArgs},
    error::Result,
    output::print_json,
};

pub(crate) async fn get_job(redloop: &RedisRedloopClient, args: JobArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    print_json(&queue.get_job(&args.job_id).await?);
    Ok(())
}

pub(crate) async fn cancel(redloop: &RedisRedloopClient, args: JobArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    queue.cancel(&args.job_id).await?;
    print_ok();
    Ok(())
}

pub(crate) async fn force_ack(redloop: &RedisRedloopClient, args: JobArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    queue.force_ack(&args.job_id).await?;
    print_ok();
    Ok(())
}

pub(crate) async fn force_fail(redloop: &RedisRedloopClient, args: ForceFailArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    queue
        .force_fail(ForceFailRequest {
            job_id: args.job_id,
            message: args.message,
        })
        .await?;
    print_ok();
    Ok(())
}

pub(crate) async fn requeue(redloop: &RedisRedloopClient, args: JobArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    queue.requeue(&args.job_id).await?;
    print_ok();
    Ok(())
}

pub(crate) async fn retry_now(redloop: &RedisRedloopClient, args: JobArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    queue.retry_now(&args.job_id).await?;
    print_ok();
    Ok(())
}

pub(crate) async fn force_retry(redloop: &RedisRedloopClient, args: JobArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    queue.force_retry(&args.job_id).await?;
    print_ok();
    Ok(())
}

fn print_ok() {
    print_json(&serde_json::json!({ "status": "ok" }));
}
