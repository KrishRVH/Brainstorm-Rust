use std::thread;

use crate::seed::SEED_SPACE;

const DEFAULT_SEED_BUDGET: i64 = 100_000_000;

/// Resolve the public search budget.
///
/// The FFI ABI preserves Original Brainstorm semantics: `num_seeds <= 0` means
/// "use the default budget", not "scan zero seeds". UI callers should pass a
/// positive, user-selected budget.
pub fn resolve_seed_budget(num_seeds: i64) -> i64 {
    let budget = if num_seeds <= 0 {
        DEFAULT_SEED_BUDGET
    } else {
        num_seeds
    };
    budget.min(SEED_SPACE)
}

pub fn resolve_threads(threads: i32) -> usize {
    if threads > 0 {
        return threads.clamp(1, 4) as usize;
    }
    thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .clamp(1, 4)
}
