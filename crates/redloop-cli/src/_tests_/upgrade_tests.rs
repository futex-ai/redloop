use super::{
    DEFAULT_REDLOOP_RELEASE_SERVER_URL, build_update_check, resolve_release_server_with_env,
};

#[test]
fn build_update_check_uses_redloop_binary_version_and_target() {
    let check = build_update_check("redloop-cli", "x86_64-unknown-linux-gnu".to_owned());

    assert_eq!(check.binary, "redloop-cli");
    assert_eq!(check.current_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(check.target, "x86_64-unknown-linux-gnu");
}

#[test]
fn explicit_release_server_overrides_default() {
    let server = resolve_release_server_with_env(Some("https://example.test".to_owned()), None);

    assert_eq!(server, "https://example.test");
}

#[test]
fn default_release_server_points_to_redloop_cloud_run() {
    let server = resolve_release_server_with_env(None, None);
    assert_eq!(server, DEFAULT_REDLOOP_RELEASE_SERVER_URL);
}

#[test]
fn env_release_server_overrides_default() {
    let server = resolve_release_server_with_env(None, Some("https://env.example.test".to_owned()));
    assert_eq!(server, "https://env.example.test");
}
