use redloop::RedisRedloopClient;

use crate::{cli::NamespaceArgs, error::Result, output::print_json};

pub(crate) async fn list(redloop: &RedisRedloopClient) -> Result<()> {
    print_json(&redloop.list_namespaces().await?);
    Ok(())
}

pub(crate) async fn counts(redloop: &RedisRedloopClient, args: NamespaceArgs) -> Result<()> {
    let queue = redloop.namespace(args.namespace);
    print_json(&queue.counts().await?);
    Ok(())
}
