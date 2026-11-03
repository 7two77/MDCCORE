use crate::contract::evaluation::LifecycleStatus;

pub struct State {
    pub id: u64,
    pub source_composition: u64,
    pub criteria_met: bool,
    pub status: LifecycleStatus,
}