use alloc::vec::Vec;
pub struct Provenance {
    pub derived_by: &'static str,
    pub inputs: Vec<u64>,
}