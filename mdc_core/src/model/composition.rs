use crate::contract::evaluation::LifecycleStatus;

pub struct Composition {
    pub id: u64,
    pub members: Vec<u64>,
    pub relations: Vec<u64>,
    pub constraints_satisfied: bool,
    pub status: LifecycleStatus,
}