use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const TSV_HEADER: &str = "kind\timpl\tcase\tgroup\tshape\tbudget\tscanned\tscan_pct\tthreads\tsample\telapsed_ms\tseeds_per_sec\tns_per_seed\tmin_ms\tp50_ms\tp95_ms\tp99_ms\tmax_ms\tstdev_ms\tcv_pct\tresult";
const METRICS: [Metric; 4] = [Metric::P50, Metric::P95, Metric::P99, Metric::Mean];
const MIN_P99_SAMPLES_PER_ARM_CYCLE: usize = 1_000;
const USAGE: &str = "usage: bench_current_compare --harness PATH --baseline PATH --candidate PATH [--executor native|wine] [--native-stage-dir PATH] [--case SELECTOR] [--budget N] [--threads N] [--repeat N] [--warmup N] [--cycles N] [--min-ratio N] [--min-regression-ms N]";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Arm {
    A,
    B,
}

impl Arm {
    const fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Executor {
    Native,
    Wine,
}

impl Executor {
    const fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Wine => "wine",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Metric {
    P50,
    P95,
    P99,
    Mean,
}

impl Metric {
    const fn label(self) -> &'static str {
        match self {
            Self::P50 => "p50",
            Self::P95 => "p95",
            Self::P99 => "p99",
            Self::Mean => "mean",
        }
    }
}

#[derive(Debug)]
struct Args {
    harness: PathBuf,
    baseline: PathBuf,
    candidate: PathBuf,
    executor: Executor,
    native_stage_dir: Option<PathBuf>,
    case: String,
    budget: i64,
    threads: i32,
    repeat: usize,
    warmup: usize,
    cycles: usize,
    min_ratio: f64,
    min_regression_ms: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct Run {
    scanned: i64,
    result: String,
    elapsed_ms: f64,
}

type Runs = BTreeMap<String, Vec<Run>>;
type Samples = BTreeMap<(String, usize, Arm), Vec<Run>>;
type ArtifactPaths = BTreeMap<&'static str, PathBuf>;
type ArtifactHashes = BTreeMap<&'static str, String>;
type Failures = BTreeSet<(String, Metric)>;

#[derive(Debug)]
struct MetricRow {
    case: String,
    metric: Metric,
    scanned: i64,
    result: String,
    baseline_ms: f64,
    candidate_ms: f64,
    paired_ratio: f64,
    paired_delta_ms: f64,
    pooled_ratio: f64,
    pooled_delta_ms: f64,
    regression_cycles: usize,
    cycle_ratios: Vec<f64>,
    cycle_deltas_ms: Vec<f64>,
    status: &'static str,
}

struct TempDir {
    path: PathBuf,
    remove_on_drop: bool,
}

impl TempDir {
    fn create(root: &Path) -> Result<Self, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        for attempt in 0..100_u8 {
            let path = root.join(format!(
                "brainstorm-current-{}-{timestamp}-{attempt}",
                process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        remove_on_drop: true,
                    });
                },
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {},
                Err(err) => {
                    return Err(format!(
                        "failed to create native staging directory under {}: {err}",
                        root.display()
                    ));
                },
            }
        }
        Err(format!(
            "failed to create a unique native staging directory under {}",
            root.display()
        ))
    }

    fn remove(mut self) -> Result<(), String> {
        fs::remove_dir_all(&self.path).map_err(|err| {
            format!(
                "failed to remove native staging directory {}: {err}",
                self.path.display()
            )
        })?;
        self.remove_on_drop = false;
        Ok(())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("{USAGE}");
            eprintln!("bench-current-compare: {err}");
            process::exit(2);
        },
    };
    match compare(&args) {
        Ok(code) => process::exit(code),
        Err(err) => {
            eprintln!("bench-current-compare: {err}");
            process::exit(2);
        },
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut harness = None;
    let mut baseline = None;
    let mut candidate = None;
    let mut executor = Executor::Native;
    let mut native_stage_dir = None;
    let mut case = "ux".to_owned();
    let mut budget = 100_000;
    let mut threads = 0;
    let mut repeat = 31;
    let mut warmup = 3;
    let mut cycles = 4;
    let mut min_ratio = 0.99;
    let mut min_regression_ms = 0.005;
    let mut iter = args;

    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--harness" => harness = Some(PathBuf::from(value)),
            "--baseline" => baseline = Some(PathBuf::from(value)),
            "--candidate" => candidate = Some(PathBuf::from(value)),
            "--executor" => {
                executor = match value.as_str() {
                    "native" => Executor::Native,
                    "wine" => Executor::Wine,
                    _ => return Err(format!("invalid --executor: {value}")),
                };
            },
            "--native-stage-dir" => native_stage_dir = Some(PathBuf::from(value)),
            "--case" => case = value,
            "--budget" => budget = parse_value(&value, "--budget")?,
            "--threads" => threads = parse_value(&value, "--threads")?,
            "--repeat" => repeat = parse_value(&value, "--repeat")?,
            "--warmup" => warmup = parse_value(&value, "--warmup")?,
            "--cycles" => cycles = parse_value(&value, "--cycles")?,
            "--min-ratio" => min_ratio = parse_value(&value, "--min-ratio")?,
            "--min-regression-ms" => {
                min_regression_ms = parse_value(&value, "--min-regression-ms")?;
            },
            _ => return Err(format!("unknown argument: {flag}")),
        }
    }

    Ok(Args {
        harness: harness.ok_or_else(|| "missing --harness".to_owned())?,
        baseline: baseline.ok_or_else(|| "missing --baseline".to_owned())?,
        candidate: candidate.ok_or_else(|| "missing --candidate".to_owned())?,
        executor,
        native_stage_dir,
        case,
        budget,
        threads,
        repeat,
        warmup,
        cycles,
        min_ratio,
        min_regression_ms,
    })
}

fn parse_value<T>(value: &str, flag: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    value
        .parse()
        .map_err(|_| format!("invalid {flag}: {value}"))
}

fn compare(args: &Args) -> Result<i32, String> {
    let mut source_paths = ArtifactPaths::new();
    source_paths.insert("harness", resolve_file("harness", &args.harness)?);
    source_paths.insert("baseline", resolve_file("baseline", &args.baseline)?);
    source_paths.insert("candidate", resolve_file("candidate", &args.candidate)?);
    source_paths.insert(
        "comparator",
        resolve_file(
            "comparator",
            &env::current_exe().map_err(|err| format!("cannot locate comparator: {err}"))?,
        )?,
    );
    validate_args(args)?;

    let source_hashes = hash_artifacts(&source_paths)?;
    if source_hashes.get("baseline") == source_hashes.get("candidate") {
        return Err("baseline and candidate DLLs have identical contents".to_owned());
    }
    emit_settings(args);
    emit_artifacts("source", &source_paths, &source_hashes);
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush settings: {err}"))?;

    let collection = prepared_collection(args, &source_paths, &source_hashes);
    let source_check = verify_artifacts(&source_paths, &source_hashes);
    if source_check.is_ok() {
        println!("artifact_check\tsource\tok");
    }
    source_check?;
    let samples = collection?;

    let (rows, failures) = analyze_samples(&samples, args)?;
    println!(
        "case\tmetric\tscanned\tresult\tbaseline_median_ms\tcandidate_median_ms\tpaired_ratio\tpaired_delta_ms\tpooled_ratio\tpooled_delta_ms\tregression_cycles\tcycle_ratios\tcycle_deltas_ms\tstatus"
    );
    for row in rows {
        let cycle_ratios = row
            .cycle_ratios
            .iter()
            .map(|value| format!("{value:.6}"))
            .collect::<Vec<_>>()
            .join(",");
        let cycle_deltas = row
            .cycle_deltas_ms
            .iter()
            .map(|value| format!("{value:.6}"))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}/{}\t{}\t{}\t{}",
            row.case,
            row.metric.label(),
            row.scanned,
            row.result,
            row.baseline_ms,
            row.candidate_ms,
            row.paired_ratio,
            row.paired_delta_ms,
            row.pooled_ratio,
            row.pooled_delta_ms,
            row.regression_cycles,
            args.cycles,
            cycle_ratios,
            cycle_deltas,
            row.status,
        );
    }
    Ok(i32::from(!failures.is_empty()))
}

fn resolve_file(label: &str, path: &Path) -> Result<PathBuf, String> {
    let resolved = path
        .canonicalize()
        .map_err(|err| format!("cannot resolve {label} {}: {err}", path.display()))?;
    if !resolved.is_file() {
        return Err(format!("{label} is not a file: {}", resolved.display()));
    }
    Ok(resolved)
}

fn validate_args(args: &Args) -> Result<(), String> {
    if args.cycles == 0 || args.repeat == 0 || args.budget < 1 {
        return Err("cycles, repeat, budget, and warmup must be positive/nonnegative".to_owned());
    }
    if !args.min_ratio.is_finite() || !(0.0..=1.0).contains(&args.min_ratio) {
        return Err("minimum ratio must be between zero and one".to_owned());
    }
    if !args.min_regression_ms.is_finite() || args.min_regression_ms < 0.0 {
        return Err("minimum regression time cannot be negative".to_owned());
    }
    Ok(())
}

fn emit_settings(args: &Args) {
    let samples_per_arm_cycle = args.repeat.saturating_mul(2);
    let p99_gate = if samples_per_arm_cycle >= MIN_P99_SAMPLES_PER_ARM_CYCLE {
        "hard"
    } else {
        "report-only"
    };
    for (name, value) in [
        ("executor", args.executor.label().to_owned()),
        ("case", args.case.clone()),
        ("budget", args.budget.to_string()),
        ("threads", args.threads.to_string()),
        ("repeat", args.repeat.to_string()),
        ("warmup", args.warmup.to_string()),
        ("cycles", args.cycles.to_string()),
        ("order", "ABBA/BAAB".to_owned()),
        ("min_ratio", format!("{:?}", args.min_ratio)),
        ("min_regression_ms", format!("{:?}", args.min_regression_ms)),
        ("metrics", "p50,p95,p99,mean".to_owned()),
        (
            "p99_gate",
            format!(
                "{p99_gate};samples_per_arm_cycle={samples_per_arm_cycle};minimum={MIN_P99_SAMPLES_PER_ARM_CYCLE}"
            ),
        ),
    ] {
        println!("setting\t{name}\t{value}");
    }
}

fn sha256(path: &Path) -> Result<String, String> {
    let mut command = Command::new("sha256sum");
    command.arg("--").arg(path);
    let output = checked_output(&mut command, "sha256sum")?;
    let stdout = output_text(&output.stdout, "sha256sum stdout")?;
    let hash = stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("sha256sum emitted no digest for {}", path.display()))?;
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "sha256sum emitted an invalid digest for {}: {hash}",
            path.display()
        ));
    }
    Ok(hash.to_ascii_lowercase())
}

fn hash_artifacts(paths: &ArtifactPaths) -> Result<ArtifactHashes, String> {
    paths
        .iter()
        .map(|(&label, path)| Ok((label, sha256(path)?)))
        .collect()
}

fn verify_artifacts(paths: &ArtifactPaths, expected: &ArtifactHashes) -> Result<(), String> {
    let mut changed = Vec::new();
    for (&label, path) in paths {
        let expected_hash = expected
            .get(label)
            .ok_or_else(|| format!("missing expected hash for {label}"))?;
        if &sha256(path)? != expected_hash {
            changed.push(label);
        }
    }
    if changed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "artifact changed during comparison: {}",
            changed.join(", ")
        ))
    }
}

fn emit_artifacts(scope: &str, paths: &ArtifactPaths, hashes: &ArtifactHashes) {
    for label in ["baseline", "candidate", "harness", "comparator"] {
        if let (Some(path), Some(hash)) = (paths.get(label), hashes.get(label)) {
            println!("artifact\t{scope}\t{label}\t{hash}\t{}", path.display());
        }
    }
}

fn prepared_collection(
    args: &Args,
    source_paths: &ArtifactPaths,
    source_hashes: &ArtifactHashes,
) -> Result<Samples, String> {
    if args.executor == Executor::Wine {
        let harness = source_paths
            .get("harness")
            .ok_or_else(|| "missing harness path".to_owned())?;
        let dlls = [
            windows_path(
                source_paths
                    .get("baseline")
                    .ok_or_else(|| "missing baseline path".to_owned())?,
                Executor::Wine,
            )?,
            windows_path(
                source_paths
                    .get("candidate")
                    .ok_or_else(|| "missing candidate path".to_owned())?,
                Executor::Wine,
            )?,
        ];
        return collect_samples(args, harness, &dlls, invoke);
    }

    let stage_root = native_temp_root(args.native_stage_dir.as_deref())?;
    let stage = TempDir::create(&stage_root)?;
    let baseline_dir = stage.path.join("A");
    let candidate_dir = stage.path.join("B");
    fs::create_dir(&baseline_dir).map_err(|err| {
        format!(
            "failed to create staging directory {}: {err}",
            baseline_dir.display()
        )
    })?;
    fs::create_dir(&candidate_dir).map_err(|err| {
        format!(
            "failed to create staging directory {}: {err}",
            candidate_dir.display()
        )
    })?;
    let mut staged_paths = ArtifactPaths::new();
    staged_paths.insert("baseline", baseline_dir.join("Immolate.dll"));
    staged_paths.insert("candidate", candidate_dir.join("Immolate.dll"));
    staged_paths.insert("harness", stage.path.join("harness.exe"));
    for label in ["baseline", "candidate", "harness"] {
        fs::copy(
            source_paths
                .get(label)
                .ok_or_else(|| format!("missing source path for {label}"))?,
            staged_paths
                .get(label)
                .ok_or_else(|| format!("missing staged path for {label}"))?,
        )
        .map_err(|err| format!("failed to stage {label}: {err}"))?;
    }
    verify_artifacts(&staged_paths, source_hashes)?;
    emit_artifacts("native-staged", &staged_paths, source_hashes);
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush staged artifacts: {err}"))?;

    let collection = (|| {
        let harness = staged_paths
            .get("harness")
            .ok_or_else(|| "missing staged harness path".to_owned())?;
        let dlls = [
            windows_path(
                staged_paths
                    .get("baseline")
                    .ok_or_else(|| "missing staged baseline path".to_owned())?,
                Executor::Native,
            )?,
            windows_path(
                staged_paths
                    .get("candidate")
                    .ok_or_else(|| "missing staged candidate path".to_owned())?,
                Executor::Native,
            )?,
        ];
        collect_samples(args, harness, &dlls, invoke)
    })();
    let stage_check = verify_artifacts(&staged_paths, source_hashes);
    if stage_check.is_ok() {
        println!("artifact_check\tnative-staged\tok");
    }
    let cleanup = stage.remove();
    stage_check?;
    cleanup?;
    collection
}

fn native_temp_root(requested: Option<&Path>) -> Result<PathBuf, String> {
    let requested = if let Some(path) = requested {
        path.to_path_buf()
    } else {
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/c", "echo", "%TEMP%"]);
        let output = checked_output(&mut command, "cmd.exe %TEMP%")?;
        let windows_temp = trimmed_output(&output.stdout, "cmd.exe %TEMP% stdout")?;
        let mut convert = Command::new("wslpath");
        convert.args(["-u", &windows_temp]);
        let output = checked_output(&mut convert, "wslpath -u")?;
        PathBuf::from(trimmed_output(&output.stdout, "wslpath -u stdout")?)
    };
    let root = requested.canonicalize().map_err(|err| {
        format!(
            "cannot resolve native staging root {}: {err}",
            requested.display()
        )
    })?;
    if !root.is_dir() {
        return Err(format!(
            "native staging root is not a directory: {}",
            root.display()
        ));
    }
    if windows_path(&root, Executor::Native)?.starts_with("\\\\") {
        return Err("native staging must use a Windows-mounted local directory".to_owned());
    }
    Ok(root)
}

fn windows_path(path: &Path, executor: Executor) -> Result<String, String> {
    let program = if executor == Executor::Wine {
        "winepath"
    } else {
        "wslpath"
    };
    let mut command = Command::new(program);
    command.arg("-w").arg(path);
    let output = checked_output(&mut command, program)?;
    trimmed_output(&output.stdout, &format!("{program} stdout"))
}

fn checked_output(command: &mut Command, description: &str) -> Result<Output, String> {
    let output = command
        .output()
        .map_err(|err| format!("failed to run {description}: {err}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{description} failed with {}: {}",
            status_label(&output),
            stderr.trim()
        ))
    }
}

fn output_text<'a>(output: &'a [u8], description: &str) -> Result<&'a str, String> {
    std::str::from_utf8(output).map_err(|err| format!("{description} is not UTF-8: {err}"))
}

fn trimmed_output(output: &[u8], description: &str) -> Result<String, String> {
    let value = output_text(output, description)?.trim().to_owned();
    if value.is_empty() {
        Err(format!("{description} is empty"))
    } else {
        Ok(value)
    }
}

fn status_label(output: &Output) -> String {
    output
        .status
        .code()
        .map_or_else(|| "no exit code".to_owned(), |code| code.to_string())
}

fn invoke(args: &Args, harness: &Path, dll: &str) -> Result<Runs, String> {
    let mut command = if args.executor == Executor::Wine {
        let mut command = Command::new("wine");
        command.arg(harness);
        command
    } else {
        Command::new(harness)
    };
    command
        .arg("bench")
        .arg("--dll")
        .arg(dll)
        .arg("--case")
        .arg(&args.case)
        .arg("--budget")
        .arg(args.budget.to_string())
        .arg("--threads")
        .arg(args.threads.to_string())
        .arg("--repeat")
        .arg(args.repeat.to_string())
        .arg("--warmup")
        .arg(args.warmup.to_string())
        .args(["--format", "tsv", "--color", "never"]);
    let output = command
        .output()
        .map_err(|err| format!("failed to run DLL harness: {err}"))?;
    if !output.status.success() {
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(&output.stdout);
        let _ = stderr.write_all(&output.stderr);
        return Err(format!(
            "DLL harness failed with exit {}",
            status_label(&output)
        ));
    }
    parse_runs(
        output_text(&output.stdout, "DLL harness stdout")?,
        args.repeat,
        Some(args.budget),
        Some(args.threads),
    )
}

fn parse_runs(
    output: &str,
    repeat: usize,
    expected_budget: Option<i64>,
    expected_threads: Option<i32>,
) -> Result<Runs, String> {
    let mut lines = output.lines();
    if lines.next() != Some(TSV_HEADER) {
        return Err("DLL harness emitted malformed TSV".to_owned());
    }
    let mut runs = Runs::new();
    let mut sample_ids: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut summaries = BTreeSet::new();
    for line in lines {
        let row = line.split('\t').collect::<Vec<_>>();
        if row.len() != 21 {
            return Err("DLL harness emitted malformed TSV".to_owned());
        }
        let case = row[2].to_owned();
        match row[0] {
            "run" => {
                if expected_budget
                    .is_some_and(|budget| parse_value::<i64>(row[5], "budget") != Ok(budget))
                {
                    return Err(format!("budget mismatch for {case}"));
                }
                if expected_threads
                    .is_some_and(|threads| parse_value::<i32>(row[8], "threads") != Ok(threads))
                {
                    return Err(format!("thread mismatch for {case}"));
                }
                let elapsed_ms = parse_value::<f64>(row[10], "elapsed time")?;
                if !elapsed_ms.is_finite() || elapsed_ms < 0.0 {
                    return Err(format!("invalid elapsed time for {case}: {}", row[10]));
                }
                let sample = parse_value::<usize>(row[9], "sample")?;
                sample_ids.entry(case.clone()).or_default().insert(sample);
                runs.entry(case).or_default().push(Run {
                    scanned: parse_value(row[6], "scanned count")?,
                    result: row[20].to_owned(),
                    elapsed_ms,
                });
            },
            "summary" => {
                if parse_value::<usize>(row[9], "summary repeat")? != repeat {
                    return Err(format!("repeat mismatch for {case}"));
                }
                if !summaries.insert(case.clone()) {
                    return Err(format!("duplicate summary for {case}"));
                }
            },
            kind => return Err(format!("unexpected harness row: {kind}")),
        }
    }
    if runs.is_empty() || runs.keys().cloned().collect::<BTreeSet<_>>() != summaries {
        return Err("run/summary case coverage mismatch".to_owned());
    }
    let expected_ids = (1..=repeat).collect::<BTreeSet<_>>();
    for (case, samples) in &runs {
        let identities = samples
            .iter()
            .map(|sample| (sample.scanned, sample.result.as_str()))
            .collect::<BTreeSet<_>>();
        if samples.len() != repeat
            || sample_ids.get(case) != Some(&expected_ids)
            || identities.len() != 1
        {
            return Err(format!("unstable result/scanned count for {case}"));
        }
    }
    Ok(runs)
}

fn collect_samples<F>(
    args: &Args,
    harness: &Path,
    dlls: &[String; 2],
    mut invoke_fn: F,
) -> Result<Samples, String>
where
    F: FnMut(&Args, &Path, &str) -> Result<Runs, String>,
{
    let mut samples = Samples::new();
    for cycle in 1..=args.cycles {
        let order = if cycle % 2 == 1 {
            [Arm::A, Arm::B, Arm::B, Arm::A]
        } else {
            [Arm::B, Arm::A, Arm::A, Arm::B]
        };
        for arm in order {
            for (case, runs) in invoke_fn(args, harness, &dlls[arm.index()])? {
                samples.entry((case, cycle, arm)).or_default().extend(runs);
            }
        }
    }
    Ok(samples)
}

fn analyze_samples(samples: &Samples, args: &Args) -> Result<(Vec<MetricRow>, Failures), String> {
    let cases = samples
        .keys()
        .map(|(case, _, _)| case.clone())
        .collect::<BTreeSet<_>>();
    let expected_per_arm = args
        .repeat
        .checked_mul(2)
        .ok_or_else(|| "repeat is too large".to_owned())?;
    let mut rows = Vec::new();
    let mut failures = BTreeSet::new();
    for case in cases {
        let mut identities = BTreeSet::new();
        let mut by_cycle = Vec::with_capacity(args.cycles);
        for cycle in 1..=args.cycles {
            let mut arms = [Vec::new(), Vec::new()];
            for arm in [Arm::A, Arm::B] {
                let runs = samples
                    .get(&(case.clone(), cycle, arm))
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                if runs.len() != expected_per_arm {
                    return Err(format!(
                        "sample coverage mismatch for {case}/cycle {cycle}/{arm:?}"
                    ));
                }
                for run in runs {
                    identities.insert((run.scanned, run.result.clone()));
                    arms[arm.index()].push(run.elapsed_ms);
                }
            }
            by_cycle.push(arms);
        }
        if identities.len() != 1 {
            return Err(format!(
                "baseline/candidate result mismatch for {case}: {identities:?}"
            ));
        }
        let (scanned, result) = identities
            .into_iter()
            .next()
            .ok_or_else(|| format!("missing result identity for {case}"))?;

        for metric in METRICS {
            let cycle_values = by_cycle
                .iter()
                .map(|arms| {
                    Ok((
                        metric_value(&arms[Arm::A.index()], metric)?,
                        metric_value(&arms[Arm::B.index()], metric)?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let baseline_ms = median(cycle_values.iter().map(|values| values.0))?;
            let candidate_ms = median(cycle_values.iter().map(|values| values.1))?;
            let cycle_ratios = cycle_values
                .iter()
                .map(|&(baseline, candidate)| ratio(baseline, candidate))
                .collect::<Vec<_>>();
            let cycle_deltas_ms = cycle_values
                .iter()
                .map(|&(baseline, candidate)| candidate - baseline)
                .collect::<Vec<_>>();
            let paired_ratio = median(cycle_ratios.iter().copied())?;
            let paired_delta_ms = median(cycle_deltas_ms.iter().copied())?;
            let pooled_a = metric_value(
                &by_cycle
                    .iter()
                    .flat_map(|arms| arms[Arm::A.index()].iter().copied())
                    .collect::<Vec<_>>(),
                metric,
            )?;
            let pooled_b = metric_value(
                &by_cycle
                    .iter()
                    .flat_map(|arms| arms[Arm::B.index()].iter().copied())
                    .collect::<Vec<_>>(),
                metric,
            )?;
            let regression_cycles = cycle_values
                .iter()
                .filter(|&&(baseline, candidate)| {
                    is_regression(baseline, candidate, args.min_ratio, args.min_regression_ms)
                })
                .count();
            let paired_regression =
                paired_delta_ms > args.min_regression_ms && paired_ratio < args.min_ratio;
            let pooled_regression =
                is_regression(pooled_a, pooled_b, args.min_ratio, args.min_regression_ms);
            let detected_regression =
                regression_cycles > args.cycles / 2 || (paired_regression && pooled_regression);
            let failed = detected_regression
                && (metric != Metric::P99 || expected_per_arm >= MIN_P99_SAMPLES_PER_ARM_CYCLE);
            if failed {
                failures.insert((case.clone(), metric));
            }
            let watch = regression_cycles > 0 || paired_regression || pooled_regression;
            rows.push(MetricRow {
                case: case.clone(),
                metric,
                scanned,
                result: result.clone(),
                baseline_ms,
                candidate_ms,
                paired_ratio,
                paired_delta_ms,
                pooled_ratio: ratio(pooled_a, pooled_b),
                pooled_delta_ms: pooled_b - pooled_a,
                regression_cycles,
                cycle_ratios,
                cycle_deltas_ms,
                status: if failed {
                    "regression"
                } else if watch {
                    "watch"
                } else {
                    "ok"
                },
            });
        }
    }
    Ok((rows, failures))
}

fn metric_value(values: &[f64], metric: Metric) -> Result<f64, String> {
    match metric {
        Metric::Mean => {
            if values.is_empty() {
                Err("cannot calculate mean of empty samples".to_owned())
            } else {
                Ok(precise_sum(values)? / values.len() as f64)
            }
        },
        Metric::P50 => percentile(values, 0.50),
        Metric::P95 => percentile(values, 0.95),
        Metric::P99 => percentile(values, 0.99),
    }
}

fn precise_sum(values: &[f64]) -> Result<f64, String> {
    let mut partials = Vec::<f64>::new();
    for &value in values {
        let mut x = value;
        let mut retained = 0;
        for index in 0..partials.len() {
            let mut y = partials[index];
            if x.abs() < y.abs() {
                std::mem::swap(&mut x, &mut y);
            }
            let high = x + y;
            let low = y - (high - x);
            if low != 0.0 {
                partials[retained] = low;
                retained += 1;
            }
            x = high;
        }
        partials.truncate(retained);
        if x != 0.0 {
            if !x.is_finite() {
                return Err("intermediate overflow while calculating mean".to_owned());
            }
            partials.push(x);
        }
    }

    let Some(mut high) = partials.pop() else {
        return Ok(0.0);
    };
    let mut low = 0.0;
    while let Some(value) = partials.pop() {
        let previous = high;
        high = previous + value;
        low = value - (high - previous);
        if low != 0.0 {
            break;
        }
    }
    if partials
        .last()
        .is_some_and(|next| (low < 0.0 && *next < 0.0) || (low > 0.0 && *next > 0.0))
    {
        let correction = low * 2.0;
        let corrected = high + correction;
        if corrected - high == correction {
            high = corrected;
        }
    }
    Ok(high)
}

fn percentile(values: &[f64], pct: f64) -> Result<f64, String> {
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    let index = ((ordered.len().saturating_sub(1)) as f64 * pct).ceil() as usize;
    ordered
        .get(index)
        .copied()
        .ok_or_else(|| "cannot calculate percentile of empty samples".to_owned())
}

fn median(values: impl Iterator<Item = f64>) -> Result<f64, String> {
    let mut ordered = values.collect::<Vec<_>>();
    ordered.sort_by(f64::total_cmp);
    let middle = ordered.len() / 2;
    let upper = ordered
        .get(middle)
        .copied()
        .ok_or_else(|| "cannot calculate median of empty samples".to_owned())?;
    if ordered.len() % 2 == 0 {
        let lower = ordered
            .get(middle - 1)
            .copied()
            .ok_or_else(|| "cannot calculate median of empty samples".to_owned())?;
        Ok((lower + upper) / 2.0)
    } else {
        Ok(upper)
    }
}

fn is_regression(
    baseline_ms: f64,
    candidate_ms: f64,
    min_ratio: f64,
    min_regression_ms: f64,
) -> bool {
    candidate_ms - baseline_ms > min_regression_ms
        && candidate_ms > 0.0
        && baseline_ms / candidate_ms < min_ratio
}

fn ratio(baseline_ms: f64, candidate_ms: f64) -> f64 {
    if candidate_ms == 0.0 {
        f64::INFINITY
    } else {
        baseline_ms / candidate_ms
    }
}

#[cfg(test)]
fn synthetic_tsv(times: &[f64], result: &str) -> String {
    let mut output = String::from(TSV_HEADER);
    output.push('\n');
    for (index, elapsed) in times.iter().enumerate() {
        let mut row = vec![String::new(); 21];
        "run".clone_into(&mut row[0]);
        "synthetic".clone_into(&mut row[2]);
        "10".clone_into(&mut row[6]);
        row[9] = (index + 1).to_string();
        row[10] = elapsed.to_string();
        result.clone_into(&mut row[20]);
        output.push_str(&row.join("\t"));
        output.push('\n');
    }
    let mut summary = vec![String::new(); 21];
    "summary".clone_into(&mut summary[0]);
    "synthetic".clone_into(&mut summary[2]);
    summary[9] = times.len().to_string();
    output.push_str(&summary.join("\t"));
    output.push('\n');
    output
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{
        Args, Arm, Executor, Metric, Run, Samples, TSV_HEADER, TempDir, analyze_samples,
        collect_samples, hash_artifacts, is_regression, metric_value, parse_runs, synthetic_tsv,
        verify_artifacts,
    };

    fn args(cycles: usize, repeat: usize) -> Args {
        Args {
            harness: PathBuf::new(),
            baseline: PathBuf::new(),
            candidate: PathBuf::new(),
            executor: Executor::Native,
            native_stage_dir: None,
            case: "synthetic".to_owned(),
            budget: 100_000,
            threads: 0,
            repeat,
            warmup: 0,
            cycles,
            min_ratio: 0.99,
            min_regression_ms: 0.005,
        }
    }

    fn repeated_run(elapsed_ms: f64, count: usize) -> Vec<Run> {
        vec![
            Run {
                scanned: 10,
                result: "SEED".to_owned(),
                elapsed_ms,
            };
            count
        ]
    }

    #[test]
    fn parses_strict_harness_tsv() {
        let parsed = parse_runs(&synthetic_tsv(&[1.0, 1.1], "SEED"), 2, None, None);
        assert_eq!(
            parsed.as_ref().ok().and_then(|runs| runs.get("synthetic")),
            Some(&vec![
                Run {
                    scanned: 10,
                    result: "SEED".to_owned(),
                    elapsed_ms: 1.0,
                },
                Run {
                    scanned: 10,
                    result: "SEED".to_owned(),
                    elapsed_ms: 1.1,
                },
            ])
        );
        assert!(parse_runs("bad\n", 1, None, None).is_err());
        let valid = synthetic_tsv(&[1.0], "SEED");
        let trailing_space = valid.replacen(TSV_HEADER, &format!("{TSV_HEADER} "), 1);
        let trailing_tab = valid.replacen(TSV_HEADER, &format!("{TSV_HEADER}\t"), 1);
        assert!(parse_runs(&trailing_space, 1, None, None).is_err());
        assert!(parse_runs(&trailing_tab, 1, None, None).is_err());
    }

    #[test]
    fn counterbalances_arms_and_detects_identity_mismatches() {
        let args = args(2, 2);
        let mut calls = Vec::new();
        let dlls = ["A".to_owned(), "B".to_owned()];
        let samples = collect_samples(&args, Path::new("harness"), &dlls, |_, _, dll| {
            calls.push(dll.to_owned());
            let elapsed = if dll == "A" { 1.0 } else { 1.02 };
            Ok([("synthetic".to_owned(), repeated_run(elapsed, 2))].into())
        });
        assert_eq!(calls, ["A", "B", "B", "A", "B", "A", "A", "B"]);
        let samples = samples.unwrap_or_default();
        let analyzed = analyze_samples(&samples, &args);
        assert_eq!(analyzed.as_ref().ok().map(|(rows, _)| rows.len()), Some(4));
        assert_eq!(
            analyzed.as_ref().ok().map(|(_, failures)| failures.len()),
            Some(3)
        );
        assert!(analyzed.as_ref().is_ok_and(|(rows, failures)| {
            !failures.contains(&("synthetic".to_owned(), Metric::P99))
                && rows
                    .iter()
                    .any(|row| row.metric == Metric::P99 && row.status == "watch")
        }));

        let mut mismatched = samples;
        if let Some(runs) = mismatched.get_mut(&("synthetic".to_owned(), 1, Arm::B))
            && let Some(run) = runs.first_mut()
        {
            run.result = "DIFFERENT".to_owned();
        }
        assert!(analyze_samples(&mismatched, &args).is_err());
    }

    #[test]
    fn majority_cycles_fail_but_minority_cycles_only_watch() {
        let mut args = args(3, 2);
        let mut samples = Samples::new();
        for cycle in 1..=3 {
            samples.insert(("majority".to_owned(), cycle, Arm::A), repeated_run(1.0, 4));
            samples.insert(
                ("majority".to_owned(), cycle, Arm::B),
                repeated_run(if cycle < 3 { 1.02 } else { 0.99 }, 4),
            );
        }
        let majority = analyze_samples(&samples, &args);
        assert!(majority.as_ref().is_ok_and(|(_, failures)| {
            failures.contains(&("majority".to_owned(), Metric::P50))
        }));
        samples.insert(("majority".to_owned(), 2, Arm::B), repeated_run(0.99, 4));
        let minority = analyze_samples(&samples, &args);
        assert!(minority.as_ref().is_ok_and(|(_, failures)| {
            !failures.contains(&("majority".to_owned(), Metric::P50))
        }));

        args.cycles = 4;
        args.repeat = 1;
        let mut drift = Samples::new();
        for (cycle, (baseline, candidate)) in
            [(100.0, 102.0), (100.0, 102.0), (1.0, 0.99), (1.0, 0.99)]
                .into_iter()
                .enumerate()
        {
            drift.insert(
                ("drift".to_owned(), cycle + 1, Arm::A),
                repeated_run(baseline, 2),
            );
            drift.insert(
                ("drift".to_owned(), cycle + 1, Arm::B),
                repeated_run(candidate, 2),
            );
        }
        let analyzed = analyze_samples(&drift, &args);
        assert!(analyzed.as_ref().is_ok_and(|(rows, failures)| {
            !failures.contains(&("drift".to_owned(), Metric::P50))
                && rows
                    .iter()
                    .any(|row| row.metric == Metric::P50 && row.status == "watch")
        }));
    }

    #[test]
    fn tail_metrics_catch_regressions_hidden_from_the_median() {
        let args = args(3, 20);
        let mut samples = Samples::new();
        for cycle in 1..=3 {
            let baseline = [vec![1.0; 38], vec![2.0; 2]].concat();
            let candidate = [vec![1.0; 38], vec![3.0; 2]].concat();
            samples.insert(
                ("tail".to_owned(), cycle, Arm::A),
                baseline
                    .into_iter()
                    .map(|elapsed_ms| Run {
                        scanned: 10,
                        result: "SEED".to_owned(),
                        elapsed_ms,
                    })
                    .collect(),
            );
            samples.insert(
                ("tail".to_owned(), cycle, Arm::B),
                candidate
                    .into_iter()
                    .map(|elapsed_ms| Run {
                        scanned: 10,
                        result: "SEED".to_owned(),
                        elapsed_ms,
                    })
                    .collect(),
            );
        }
        let analyzed = analyze_samples(&samples, &args);
        assert!(analyzed.as_ref().is_ok_and(|(rows, failures)| {
            !failures.contains(&("tail".to_owned(), Metric::P50))
                && failures.contains(&("tail".to_owned(), Metric::P95))
                && !failures.contains(&("tail".to_owned(), Metric::P99))
                && rows
                    .iter()
                    .any(|row| row.metric == Metric::P95 && row.status == "regression")
                && rows
                    .iter()
                    .any(|row| row.metric == Metric::P99 && row.status == "watch")
        }));
    }

    #[test]
    fn default_sample_count_reports_max_as_p99_without_hard_failing() {
        let values = (0_u32..62).map(f64::from).collect::<Vec<_>>();
        assert_eq!(metric_value(&values, Metric::P99), Ok(61.0));

        let args = args(4, 31);
        let mut samples = Samples::new();
        for cycle in 1..=args.cycles {
            let baseline = repeated_run(1.0, 62);
            let mut candidate = baseline.clone();
            if let Some(outlier) = candidate.last_mut() {
                outlier.elapsed_ms = 1.1;
            }
            samples.insert(("tail".to_owned(), cycle, Arm::A), baseline);
            samples.insert(("tail".to_owned(), cycle, Arm::B), candidate);
        }

        let analyzed = analyze_samples(&samples, &args);
        assert!(analyzed.as_ref().is_ok_and(|(rows, failures)| {
            failures.is_empty()
                && rows
                    .iter()
                    .any(|row| row.metric == Metric::P99 && row.status == "watch")
        }));
    }

    #[test]
    fn sufficiently_sampled_p99_regression_hard_fails() {
        for repeat in [500, 501] {
            let args = args(3, repeat);
            let sample_count = repeat * 2;
            let mut samples = Samples::new();
            for cycle in 1..=args.cycles {
                let baseline = repeated_run(1.0, sample_count);
                let mut candidate = baseline.clone();
                for tail in candidate.iter_mut().rev().take(12) {
                    tail.elapsed_ms = 1.02;
                }
                samples.insert(("tail".to_owned(), cycle, Arm::A), baseline);
                samples.insert(("tail".to_owned(), cycle, Arm::B), candidate);
            }

            let analyzed = analyze_samples(&samples, &args);
            assert!(analyzed.as_ref().is_ok_and(|(rows, failures)| {
                failures.contains(&("tail".to_owned(), Metric::P99))
                    && rows
                        .iter()
                        .any(|row| row.metric == Metric::P99 && row.status == "regression")
            }));
        }
    }

    #[test]
    fn artifact_verification_detects_mutation() {
        let temp = TempDir::create(&std::env::temp_dir());
        assert!(temp.is_ok());
        let temp = temp.ok();
        assert!(temp.is_some());
        if let Some(temp) = temp {
            let artifact = temp.path.join("artifact");
            assert!(fs::write(&artifact, b"before").is_ok());
            let paths = [("artifact", artifact.clone())].into();
            let hashes = hash_artifacts(&paths);
            assert!(hashes.is_ok());
            let hashes = hashes.unwrap_or_default();
            assert!(verify_artifacts(&paths, &hashes).is_ok());
            assert!(fs::write(&artifact, b"after").is_ok());
            assert!(verify_artifacts(&paths, &hashes).is_err());
            assert!(temp.remove().is_ok());
        }
    }

    #[test]
    fn absolute_noise_floor_prevents_false_regressions() {
        assert!(!is_regression(1.0, 1.004, 0.99, 0.005));
        assert!(is_regression(1.0, 1.02, 0.99, 0.005));
    }

    #[test]
    fn mean_matches_fsum_and_is_order_independent() {
        let samples = [1.0e16, 1.0, 1.0];
        let naive = samples.iter().sum::<f64>() / samples.len() as f64;
        let compensated = metric_value(&samples, Metric::Mean).unwrap_or_default();
        assert!(compensated > naive);

        let samples = [0.382, 1_501_575_908_433_872.5, 0.019, 1.974];
        let expected = 375_393_977_108_468.687_5;
        assert_eq!(metric_value(&samples, Metric::Mean), Ok(expected));
        assert_eq!(
            metric_value(
                &[samples[3], samples[2], samples[1], samples[0]],
                Metric::Mean
            ),
            Ok(expected)
        );
        assert_eq!(
            metric_value(
                &[samples[1], samples[3], samples[0], samples[2]],
                Metric::Mean
            ),
            Ok(expected)
        );
        assert!(metric_value(&[f64::MAX, f64::MAX], Metric::Mean).is_err());
    }
}
