use std::env;
use std::hint::black_box;
use std::process;
use std::time::Instant;

use immolate::brainstorm_search_core;
use immolate::filters::FilterConfig;
use immolate::seed::{SEED_SPACE, Seed};

#[path = "../bench_cases.rs"]
mod bench_cases;

#[derive(Debug)]
struct Args {
    case: String,
    budget: i64,
    threads: i32,
    repeat: usize,
    warmup: usize,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            case: "all".to_owned(),
            budget: 1_000_000,
            threads: 0,
            repeat: 3,
            warmup: 0,
        }
    }
}

fn main() {
    let args = parse_args().unwrap_or_else(|message| {
        eprintln!("{message}");
        eprintln!(
            "usage: brainstorm_bench [--case all|GROUP|NAME] [--budget N] [--threads N] [--repeat N] [--warmup N]"
        );
        process::exit(2);
    });

    let cases = bench_cases::selected_bench_cases(&args.case).unwrap_or_else(|err| {
        eprintln!("{err}");
        process::exit(2);
    });

    for case in cases.iter().copied() {
        let cfg = case_config(case);
        for _ in 0..args.warmup {
            black_box(brainstorm_search_core(
                case.seed_start,
                &cfg,
                args.budget,
                args.threads,
            ));
        }
    }

    println!(
        "engine\tcase\tgroup\tshape\tbudget\tscanned\tscan_pct\tthreads\trepeat\telapsed_ms\tseeds_per_sec\tns_per_seed\tresult\tnote"
    );
    for case in cases {
        let cfg = case_config(case);
        for repeat in 1..=args.repeat {
            let started = Instant::now();
            let result = brainstorm_search_core(case.seed_start, &cfg, args.budget, args.threads);
            let elapsed = started.elapsed();
            let elapsed_secs = elapsed.as_secs_f64();
            let scanned = scanned_count(case.seed_start, result.as_deref(), args.budget);
            let seeds_per_sec = if elapsed_secs > 0.0 {
                scanned as f64 / elapsed_secs
            } else {
                f64::INFINITY
            };
            let result = match result.as_deref() {
                Some("") | None => "<null>",
                Some(seed) => seed,
            };
            println!(
                "current\t{}\t{}\t{}\t{}\t{}\t{:.6}\t{}\t{}\t{:.3}\t{:.0}\t{:.3}\t{}\t{}",
                case.name,
                case.group.label(),
                case.shape.label(),
                args.budget,
                scanned,
                scanned as f64 / args.budget as f64,
                args.threads,
                repeat,
                elapsed.as_secs_f64() * 1000.0,
                seeds_per_sec,
                elapsed_secs * 1_000_000_000.0 / scanned as f64,
                result,
                case.note,
            );
        }
    }
}

fn case_config(case: bench_cases::BenchCase) -> FilterConfig {
    FilterConfig::from_raw(
        case.voucher,
        case.pack,
        case.tag1,
        case.tag2,
        case.joker,
        case.joker_location,
        case.souls,
        case.observatory,
        case.perkeo,
        case.deck,
        case.erratic,
        case.no_faces,
        case.min_face_cards,
        case.suit_ratio,
    )
}

fn scanned_count(seed_start: &str, result: Option<&str>, budget: i64) -> i64 {
    let Some(result) = result else {
        return budget;
    };
    if result.is_empty() {
        return 1;
    }
    ((Seed::from_str(result).id() - Seed::from_str(seed_start).id()).rem_euclid(SEED_SPACE) + 1)
        .min(budget)
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut iter = env::args().skip(1);
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--case" => args.case = value,
            "--budget" => {
                args.budget = value
                    .parse::<i64>()
                    .map_err(|_| format!("invalid --budget: {value}"))?;
                if args.budget <= 0 {
                    return Err("--budget must be positive".to_owned());
                }
            },
            "--threads" => {
                args.threads = value
                    .parse::<i32>()
                    .map_err(|_| format!("invalid --threads: {value}"))?;
            },
            "--repeat" => {
                args.repeat = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --repeat: {value}"))?;
                if args.repeat == 0 {
                    return Err("--repeat must be positive".to_owned());
                }
            },
            "--warmup" => {
                args.warmup = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --warmup: {value}"))?;
            },
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }
    Ok(args)
}
