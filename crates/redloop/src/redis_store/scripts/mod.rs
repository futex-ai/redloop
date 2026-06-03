//! Embedded Redis Lua programs.

pub(crate) const ACK: &str = include_str!("ack.lua");
pub(crate) const COMPLETE_AND_RESCHEDULE: &str = include_str!("complete_and_reschedule.lua");
pub(crate) const ENQUEUE_OR_RESCHEDULE: &str = include_str!("enqueue_or_reschedule.lua");
pub(crate) const FAIL_OR_RETRY: &str = include_str!("fail_or_retry.lua");
pub(crate) const HEARTBEAT: &str = include_str!("heartbeat.lua");
pub(crate) const OPERATOR_TRANSITION: &str = include_str!("operator_transition.lua");
pub(crate) const REAP_EXPIRED: &str = include_str!("reap_expired.lua");
pub(crate) const RESERVE: &str = include_str!("reserve.lua");
