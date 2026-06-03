use std::process::Command;

fn redloop_binary() -> &'static str {
    env!("CARGO_BIN_EXE_redloop-cli")
}

#[test]
fn redloop_cli_exposes_queue_and_upgrade_commands() {
    let help = run_success(["--help"]);
    for expected in [
        "namespaces",
        "counts",
        "get-job",
        "list-failed",
        "list-workers",
        "cancel",
        "force-ack",
        "force-fail",
        "requeue",
        "retry-now",
        "force-retry",
        "purge-failed",
        "upgrade",
    ] {
        assert!(
            help.contains(expected),
            "redloop-cli help should contain {expected}:\n{help}"
        );
    }

    let upgrade_help = run_success(["upgrade", "--help"]);
    assert!(upgrade_help.contains("status"));
    assert!(upgrade_help.contains("--release-server"));

    let update_help = run_success(["update", "--help"]);
    assert!(update_help.contains("status"));
    assert!(update_help.contains("--release-server"));
}

fn run_success<const N: usize>(args: [&str; N]) -> String {
    let output = Command::new(redloop_binary())
        .args(args)
        .output()
        .expect("redloop-cli command should run");
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout should be utf8")
}
