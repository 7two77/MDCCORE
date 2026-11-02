use crate::contract::evaluation::LifecycleStatus;

pub struct Transition {
    pub id: u64,
    pub source_state: u64,
    pub target_state: u64,
    pub condition_met: bool,
    pub status: LifecycleStatus,
}