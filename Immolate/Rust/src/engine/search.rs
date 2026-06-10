use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::engine::config::{CompiledFilter, KernelShape};
use crate::engine::cuda::{CudaSearch, search_cuda};
use crate::engine::kernels::apply_compiled_filter;
use crate::engine::seed::SearchState;
use crate::filters::FilterConfig;
use crate::search::{resolve_seed_budget, resolve_threads};
use crate::seed::{SEED_SPACE, Seed};

pub fn brainstorm_search_core(
    seed_start: &str,
    cfg: &FilterConfig,
    num_seeds: i64,
    threads: i32,
) -> Option<String> {
    let budget = resolve_seed_budget(num_seeds);
    let compiled = CompiledFilter::compile(cfg);
    match search_cuda(seed_start, &compiled, budget) {
        CudaSearch::Complete(result) => return result,
        CudaSearch::Unsupported | CudaSearch::Unavailable => {},
    }
    let thread_count = resolve_threads_for_engine(budget, threads, compiled.shape);
    search_filters(seed_start, compiled, budget, thread_count)
}

fn resolve_threads_for_engine(num_seeds: i64, threads: i32, shape: KernelShape) -> usize {
    if shape != KernelShape::Composite {
        return resolve_threads(threads);
    }
    if threads > 0 {
        return (threads as usize).clamp(1, 16);
    }
    if num_seeds < 8_192 {
        return 1;
    }
    thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .clamp(1, 16)
}

fn search_filters(
    seed_start: &str,
    cfg: CompiledFilter,
    num_seeds: i64,
    threads: usize,
) -> Option<String> {
    let start_seed = Seed::from_str(seed_start).id();
    if cfg.shape == KernelShape::NoMatch {
        return None;
    }
    if threads <= 1 {
        return search_block(start_seed, num_seeds, &cfg);
    }
    let block_size = cfg.chunk_size();
    let prefix_count = block_size.min(num_seeds);
    if let Some(seed) = search_block(start_seed, prefix_count, &cfg) {
        return Some(seed);
    }
    if prefix_count == num_seeds {
        return None;
    }
    let remaining_start = (start_seed + prefix_count).rem_euclid(SEED_SPACE);
    search_filters_exact_parallel(remaining_start, num_seeds - prefix_count, threads, &cfg)
}

fn search_filters_exact_parallel(
    start_seed: i64,
    num_seeds: i64,
    threads: usize,
    cfg: &CompiledFilter,
) -> Option<String> {
    let block_size = cfg.chunk_size();
    let total_blocks = (num_seeds + block_size - 1) / block_size;
    let next_block = Arc::new(AtomicI64::new(0));
    let best_block = Arc::new(AtomicI64::new(total_blocks));
    let result = Arc::new(Mutex::new(None::<(i64, String)>));

    thread::scope(|scope| {
        for _ in 0..threads {
            let next_block = Arc::clone(&next_block);
            let best_block = Arc::clone(&best_block);
            let result = Arc::clone(&result);
            scope.spawn(move || {
                loop {
                    let block = next_block.fetch_add(1, Ordering::Relaxed);
                    if block >= total_blocks || block > best_block.load(Ordering::Relaxed) {
                        break;
                    }
                    let offset = block * block_size;
                    let start = (start_seed + offset).rem_euclid(SEED_SPACE);
                    let count = block_size.min(num_seeds - offset);
                    if let Some((local_offset, seed)) = search_block_indexed(start, count, cfg) {
                        let found_offset = offset + local_offset;
                        best_block.fetch_min(block, Ordering::Relaxed);
                        if let Ok(mut guard) = result.lock() {
                            if guard
                                .as_ref()
                                .is_none_or(|(best_offset, _)| found_offset < *best_offset)
                            {
                                *guard = Some((found_offset, seed));
                            }
                        }
                    }
                }
            });
        }
    });

    result
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|(_, seed)| seed.clone()))
}

fn search_block(start: i64, count: i64, cfg: &CompiledFilter) -> Option<String> {
    let mut state = SearchState::from_id(start);
    for _ in 0..count {
        if apply_compiled_filter(&mut state, cfg) {
            return Some(state.seed.to_string());
        }
        state.next();
    }
    None
}

fn search_block_indexed(start: i64, count: i64, cfg: &CompiledFilter) -> Option<(i64, String)> {
    let mut state = SearchState::from_id(start);
    for offset in 0..count {
        if apply_compiled_filter(&mut state, cfg) {
            return Some((offset, state.seed.to_string()));
        }
        state.next();
    }
    None
}
