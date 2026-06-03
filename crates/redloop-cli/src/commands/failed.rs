use redloop::{FailedJobsQuery, RedisRedloopClient};

use crate::{cli::ListFailedArgs, cli::PurgeFailedArgs, error::Result, output::print_json};

pub(crate) async fn list(redloop: &RedisRedloopClient, args: ListFailedArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    print_json(
        &queue
            .list_failed(FailedJobsQuery {
                cursor: args.cursor,
                limit: args.limit,
            })
            .await?,
    );
    Ok(())
}

pub(crate) async fn purge(redloop: &RedisRedloopClient, args: PurgeFailedArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace.clone());
    let selector = args.selector()?;
    let deleted = queue.purge_failed(selector).await?;
    print_json(&serde_json::json!({ "deleted": deleted }));
    Ok(())
}
