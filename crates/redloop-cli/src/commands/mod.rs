mod failed;
mod jobs;
mod namespaces;
mod workers;

use redloop::RedisRedloopClient;

use crate::{
    cli::{Command, ConnectArgs},
    error::Result,
};

pub(crate) async fn run_command(connect: ConnectArgs, command: Command) -> Result<()> {
    let redloop = RedisRedloopClient::connect(connect.to_config()?).await?;
    match command {
        Command::Namespaces => namespaces::list(&redloop).await,
        Command::Counts(args) => namespaces::counts(&redloop, args).await,
        Command::GetJob(args) => jobs::get_job(&redloop, args).await,
        Command::ListFailed(args) => failed::list(&redloop, args).await,
        Command::ListWorkers(args) => workers::list(&redloop, args).await,
        Command::Cancel(args) => jobs::cancel(&redloop, args).await,
        Command::ForceAck(args) => jobs::force_ack(&redloop, args).await,
        Command::ForceFail(args) => jobs::force_fail(&redloop, args).await,
        Command::Requeue(args) => jobs::requeue(&redloop, args).await,
        Command::RetryNow(args) => jobs::retry_now(&redloop, args).await,
        Command::ForceRetry(args) => jobs::force_retry(&redloop, args).await,
        Command::PurgeFailed(args) => failed::purge(&redloop, args).await,
        Command::Upgrade(_) => unreachable!("upgrade is handled before command dispatch"),
    }
}
