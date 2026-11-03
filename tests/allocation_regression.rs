use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;

use mdc_core::canonical::snapshot::snapshot;
use mdc_core::identity::{CanonicalSemanticBasis, IdentityAdapter};
use mdc_core::model::observation::{Observation, Source};

thread_local! {
    // Tracking is local to the test thread, so allocations by the test harness
    // or another test cannot affect the measurement. Const initialization does
    // not allocate inside the allocator itself.
    static TRACKED_BYTES: Cell<Option<isize>> = const { Cell::new(None) };
}

struct TrackingAllocator;

fn record_bytes(delta: isize) {
    // Allocations during thread teardown must still reach the system allocator.
    let _ = TRACKED_BYTES.try_with(|counter| {
        if let Some(bytes) = counter.get() {
            counter.set(Some(bytes + delta));
        }
    });
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_bytes(layout.size() as isize);
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_bytes(layout.size() as isize);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record_bytes(-(layout.size() as isize));
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let replacement = unsafe { System.realloc(pointer, layout, size) };
        if !replacement.is_null() {
            record_bytes(size as isize - layout.size() as isize);
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

struct TrackingScope;

impl TrackingScope {
    fn start() -> Self {
        TRACKED_BYTES.with(|counter| assert_eq!(counter.replace(Some(0)), None));
        Self
    }

    fn finish(self) -> isize {
        TRACKED_BYTES.with(|counter| counter.replace(None).unwrap())
    }
}

impl Drop for TrackingScope {
    fn drop(&mut self) {
        TRACKED_BYTES.with(|counter| counter.set(None));
    }
}

struct TestSource;

impl Source for TestSource {
    fn id(&self) -> &str {
        "BTC"
    }
}

#[test]
fn repeated_canonicalization_identity_and_snapshots_release_their_allocations() {
    // Keep the input alive outside the measured scope. Only temporary
    // allocations made by the operations below should be counted and freed.
    let observation = Observation::new(TestSource, 100, vec![42; 1024]).unwrap();
    let tracking = TrackingScope::start();

    for _ in 0..128 {
        drop(black_box(observation.canonical_basis()));
        black_box(IdentityAdapter::derive_identity(&observation));
        drop(black_box(snapshot(&observation)));
    }

    let retained_bytes = tracking.finish();
    assert_eq!(
        retained_bytes, 0,
        "temporary canonical buffers must be freed"
    );
}
