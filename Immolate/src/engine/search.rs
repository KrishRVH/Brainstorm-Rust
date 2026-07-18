use std::sync::atomic::{AtomicI64, Ordering};
use std::thread;

use crate::engine::config::{CompiledFilter, KernelShape};
use crate::engine::kernels::apply_compiled_filter;
use crate::engine::seed::SearchState;
use crate::filters::FilterConfig;
use crate::search::resolve_seed_budget;
use crate::seed::{SEED_SPACE, Seed};

pub fn brainstorm_search_core(
    seed_start: &str,
    cfg: &FilterConfig,
    num_seeds: i64,
    threads: i32,
) -> Option<String> {
    let budget = resolve_seed_budget(num_seeds);
    let compiled = CompiledFilter::compile(cfg);
    search_filters(seed_start, compiled, budget, threads)
}

fn resolve_threads_for_engine(
    num_seeds: i64,
    threads: i32,
    maximum: usize,
    parallel_threshold: i64,
) -> usize {
    if threads > 0 {
        return (threads as usize).clamp(1, 16);
    }
    if num_seeds < parallel_threshold {
        return 1;
    }
    thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .clamp(1, maximum)
}

fn search_filters(
    seed_start: &str,
    cfg: CompiledFilter,
    num_seeds: i64,
    requested_threads: i32,
) -> Option<String> {
    if cfg.shape == KernelShape::NoMatch {
        return None;
    }
    let start_seed = Seed::from(seed_start).id();
    if cfg.shape == KernelShape::NoFilter {
        return Some(Seed::from_id(start_seed).to_string());
    }
    if requested_threads == 1 {
        return search_block(start_seed, num_seeds, &cfg);
    }
    let prefix_count = cfg.serial_prefix_size().min(num_seeds);
    if let Some(seed) = search_block(start_seed, prefix_count, &cfg) {
        return Some(seed);
    }
    if prefix_count == num_seeds {
        return None;
    }
    let remaining_start = (start_seed + prefix_count).rem_euclid(SEED_SPACE);
    let remaining = num_seeds - prefix_count;
    let thread_count = resolve_threads_for_engine(
        remaining,
        requested_threads,
        cfg.auto_thread_limit(),
        cfg.parallel_threshold(),
    );
    if thread_count <= 1 {
        return search_block(remaining_start, remaining, &cfg);
    }
    search_filters_exact_parallel(remaining_start, remaining, thread_count, &cfg)
}

fn search_filters_exact_parallel(
    start_seed: i64,
    num_seeds: i64,
    threads: usize,
    cfg: &CompiledFilter,
) -> Option<String> {
    let block_size = cfg.chunk_size();
    let total_blocks = (num_seeds + block_size - 1) / block_size;
    let next_block = AtomicI64::new(0);
    let best_offset = AtomicI64::new(num_seeds);
    let worker_count = threads.min(total_blocks as usize);
    if worker_count <= 1 {
        return search_block(start_seed, num_seeds, cfg);
    }

    thread::scope(|scope| {
        for _ in 1..worker_count {
            let next_block = &next_block;
            let best_offset = &best_offset;
            scope.spawn(move || {
                search_parallel_blocks(
                    start_seed,
                    num_seeds,
                    block_size,
                    total_blocks,
                    next_block,
                    best_offset,
                    cfg,
                );
            });
        }
        search_parallel_blocks(
            start_seed,
            num_seeds,
            block_size,
            total_blocks,
            &next_block,
            &best_offset,
            cfg,
        );
    });

    let offset = best_offset.load(Ordering::Relaxed);
    (offset < num_seeds)
        .then(|| Seed::from_id((start_seed + offset).rem_euclid(SEED_SPACE)).to_string())
}

fn search_parallel_blocks(
    start_seed: i64,
    num_seeds: i64,
    block_size: i64,
    total_blocks: i64,
    next_block: &AtomicI64,
    best_offset: &AtomicI64,
    cfg: &CompiledFilter,
) {
    loop {
        let block = next_block.fetch_add(1, Ordering::Relaxed);
        if block >= total_blocks {
            break;
        }
        let offset = block * block_size;
        if offset >= best_offset.load(Ordering::Relaxed) {
            break;
        }
        let start = (start_seed + offset).rem_euclid(SEED_SPACE);
        let count = block_size.min(num_seeds - offset);
        if let Some(local_offset) = search_block_indexed(start, count, cfg) {
            best_offset.fetch_min(offset + local_offset, Ordering::Relaxed);
        }
    }
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

fn search_block_indexed(start: i64, count: i64, cfg: &CompiledFilter) -> Option<i64> {
    let mut state = SearchState::from_id(start);
    for offset in 0..count {
        if apply_compiled_filter(&mut state, cfg) {
            return Some(offset);
        }
        state.next();
    }
    None
}
