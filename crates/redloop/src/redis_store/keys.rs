#[derive(Debug, Clone)]
pub(crate) struct NamespaceKeys {
    pub cfg: String,
    pub failures: String,
    pub ready: String,
    pub scheduled: String,
    pub leased: String,
    pub lease_meta: String,
    pub rerun: String,
    pub failed: String,
    pub workers_last_seen: String,
    pub workers_config: String,
    pub workers_leases: String,
}

pub(crate) fn namespaces_key(prefix: &str) -> String {
    if prefix.is_empty() {
        "namespaces".to_string()
    } else {
        format!("{prefix}:namespaces")
    }
}

pub(crate) fn namespace_keys(prefix: &str, namespace: &str) -> NamespaceKeys {
    let slot = namespace
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let scoped = |suffix: &str| {
        if prefix.is_empty() {
            format!("{{{slot}}}:{suffix}")
        } else {
            format!("{prefix}{{{slot}}}:{suffix}")
        }
    };

    NamespaceKeys {
        cfg: scoped("cfg"),
        failures: scoped("failures"),
        ready: scoped("ready"),
        scheduled: scoped("scheduled"),
        leased: scoped("leased"),
        lease_meta: scoped("lease_meta"),
        rerun: scoped("rerun"),
        failed: scoped("failed"),
        workers_last_seen: scoped("workers:last_seen"),
        workers_config: scoped("workers:config"),
        workers_leases: scoped("workers:leases"),
    }
}
