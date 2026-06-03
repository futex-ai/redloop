use redloop::RedisRedloopClient;

use crate::{cli::NamespaceArgs, error::Result, output::print_json};

pub(crate) async fn list(redloop: &RedisRedloopClient, args: NamespaceArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    print_json(&queue.list_workers().await?);
    Ok(())
}
