//! Heap memory of a read per staging strategy (staging plan §7.1): peak
//! heap growth during the read and the heap retained by the committed graph
//! afterwards, measured with a counting global allocator. Separate from the
//! `staging` bench so the allocator does not change its timings (and their
//! comparison with the pre-staging baseline).
//!
//! ```sh
//! cargo bench -p context-read --bench staging_memory --no-default-features --features test-api
//! ```

use std::{
    alloc::{
        GlobalAlloc,
        Layout,
        System,
    },
    sync::atomic::{
        AtomicUsize,
        Ordering::Relaxed,
    },
};

use context_read::{
    OverlayStaging,
    StagingStrategy,
    TaggedStaging,
    read_with,
};
use context_trace::positioned::{
    PositionedGraphRef,
    fixtures::{
        fixtures_dir,
        manifest,
    },
};

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(size: usize) {
    let now = CURRENT.fetch_add(size, Relaxed) + size;
    PEAK.fetch_max(now, Relaxed);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            grow(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(
        &self,
        pointer: *mut u8,
        layout: Layout,
    ) {
        unsafe { System.dealloc(pointer, layout) };
        CURRENT.fetch_sub(layout.size(), Relaxed);
    }
    unsafe fn realloc(
        &self,
        pointer: *mut u8,
        layout: Layout,
        size: usize,
    ) -> *mut u8 {
        let moved = unsafe { System.realloc(pointer, layout, size) };
        if !moved.is_null() {
            CURRENT.fetch_sub(layout.size(), Relaxed);
            grow(size);
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Peak heap growth while reading `inputs` into a fresh graph, and the heap
/// the graph retains afterwards (bytes).
fn measure<S: StagingStrategy>(inputs: &[String]) -> (usize, usize) {
    let before = CURRENT.load(Relaxed);
    PEAK.store(before, Relaxed);
    let graph = PositionedGraphRef::default();
    for input in inputs {
        read_with::<S>(&graph, input.chars());
    }
    let peak = PEAK.load(Relaxed) - before;
    let retained = CURRENT.load(Relaxed) - before;
    drop(graph);
    (peak, retained)
}

fn row<S: StagingStrategy>(
    strategy: &str,
    case: &str,
    inputs: &[Vec<String>],
) {
    let (mut peak, mut retained) = (0, 0);
    for inputs in inputs {
        let (p, r) = measure::<S>(inputs);
        peak = peak.max(p);
        retained += r;
    }
    println!(
        "| {case} | {strategy} | {:.1} | {:.1} |",
        peak as f64 / 1024.0,
        retained as f64 / 1024.0
    );
}

fn main() {
    let corpus: Vec<Vec<String>> = manifest(&fixtures_dir())
        .into_iter()
        .map(|scenario| scenario.inputs)
        .collect();
    let periodic = |n: usize| vec![vec![format!("{}x", "abc".repeat(n))]];
    println!("| Input | Strategy | Peak growth (KiB) | Retained (KiB) |");
    println!("|---|---|---|---|");
    row::<OverlayStaging>("overlay", "corpus (max peak, total retained)", &corpus);
    row::<TaggedStaging>("tagged", "corpus (max peak, total retained)", &corpus);
    for n in [64, 256, 512] {
        let case = format!("abc^{n} x");
        row::<OverlayStaging>("overlay", &case, &periodic(n));
        row::<TaggedStaging>("tagged", &case, &periodic(n));
    }
}
