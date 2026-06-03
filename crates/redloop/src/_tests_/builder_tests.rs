use crate::builder::JobBuilder;
use crate::client::client_test_support::namespace_for_tests;
use chrono::Utc;

#[test]
fn immediate_builder_converts_to_batch_item() {
    let namespace = namespace_for_tests("ns".into());
    let item = JobBuilder::new(namespace, "job-1".into()).into_batch_item();
    assert!(item.request.allow_existing_current_run);
    assert!(item.request.schedule_at.is_none());
}

#[test]
fn scheduled_builder_sets_replace_mode() {
    let namespace = namespace_for_tests("ns".into());
    let item = JobBuilder::new(namespace, "job-1".into())
        .schedule_at(Utc::now())
        .replace_if_later()
        .into_batch_item();

    assert!(!item.request.allow_existing_current_run);
    assert!(matches!(
        item.request.replace_mode,
        crate::store::ReplaceMode::IfLater
    ));
}
