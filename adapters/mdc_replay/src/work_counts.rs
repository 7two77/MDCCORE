//! Test-only observations at the real adapter call boundaries.
use std::cell::Cell;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    pub exports: usize,
    pub parses: usize,
    pub manifests: usize,
    pub restores: usize,
}
thread_local! {
    static COUNTS: Cell<Counts> = Cell::new(Counts::default());
}
pub(crate) fn bump(f: impl FnOnce(&mut Counts)) {
    COUNTS.with(|cell| {
        let mut value = cell.get();
        f(&mut value);
        cell.set(value);
    });
}
pub(crate) fn take() -> Counts {
    COUNTS.with(Cell::take)
}
