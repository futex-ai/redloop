//! Namespace catalog operations.

use redis::cmd;

use crate::error::Result;

use super::RedisStore;
use super::keys::namespaces_key;

impl RedisStore {
    pub(crate) async fn remember_namespace(&self, namespace: &str) -> Result<()> {
        let mut command = cmd("SADD");
        command
            .arg(namespaces_key(&self.config.key_prefix))
            .arg(namespace);
        let _: u64 = self
            .driver
            .query_cmd("remember_namespace", self.config.command_timeout, &command)
            .await?;
        Ok(())
    }

    pub(crate) async fn list_namespaces(&self) -> Result<Vec<String>> {
        let mut command = cmd("SMEMBERS");
        command.arg(namespaces_key(&self.config.key_prefix));
        let mut namespaces: Vec<String> = self
            .driver
            .query_cmd("list_namespaces", self.config.command_timeout, &command)
            .await?;
        namespaces.sort();
        Ok(namespaces)
    }
}
