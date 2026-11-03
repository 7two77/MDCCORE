pub struct Relation {
    pub id: u64,
    pub kind: &'static str,
    pub left_ref: u64,
    pub right_ref: u64,
}