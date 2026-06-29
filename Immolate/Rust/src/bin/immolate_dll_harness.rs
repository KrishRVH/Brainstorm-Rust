#![allow(unsafe_code)]

#[cfg(not(windows))]
fn main() {
    eprintln!("immolate_dll_harness must be built for Windows and run under Windows or Wine");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    windows_harness::main();
}

#[cfg(windows)]
#[path = "../bench_cases.rs"]
mod bench_cases;

#[cfg(windows)]
mod windows_harness {
    use std::cmp::Ordering as CmpOrdering;
    use std::env;
    use std::ffi::{CStr, CString, OsStr};
    use std::io::{self, IsTerminal};
    use std::os::raw::{c_char, c_double, c_int, c_longlong, c_void};
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use std::time::{Duration, Instant};

    use immolate::seed::{SEED_SPACE, Seed};

    use super::bench_cases::{self as bench, BenchCase, BenchGroup, BenchShape};

    type HModule = *mut c_void;
    type FarProc = *mut c_void;
    type BrainstormSearch = unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        c_double,
        bool,
        bool,
        *const c_char,
        bool,
        bool,
        c_int,
        c_double,
        c_longlong,
        c_int,
    ) -> *mut c_char;
    type OriginalBrainstorm = unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        c_double,
        bool,
        bool,
    ) -> *const c_char;
    type FreeResult = unsafe extern "C" fn(*mut c_char);
    type SetCudaEnabled = unsafe extern "C" fn(bool);

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(path: *const u16) -> HModule;
        fn GetProcAddress(module: HModule, name: *const c_char) -> FarProc;
        fn FreeLibrary(module: HModule) -> i32;
        fn CreateFileW(
            file_name: *const u16,
            desired_access: u32,
            share_mode: u32,
            security_attributes: *mut c_void,
            creation_disposition: u32,
            flags_and_attributes: u32,
            template_file: HModule,
        ) -> HModule;
        fn CloseHandle(handle: HModule) -> i32;
    }

    #[link(name = "msvcrt")]
    unsafe extern "C" {
        fn _dup(fd: c_int) -> c_int;
        fn _dup2(fd1: c_int, fd2: c_int) -> c_int;
        fn _close(fd: c_int) -> c_int;
        fn _open_osfhandle(os_file_handle: isize, flags: c_int) -> c_int;
        fn fflush(stream: *mut c_void) -> c_int;
    }

    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const OPEN_EXISTING: u32 = 3;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
    const INVALID_HANDLE_VALUE: HModule = !0_usize as HModule;
    const STDOUT_FILENO: c_int = 1;
    const O_WRONLY: c_int = 0x0001;

    #[derive(Clone)]
    struct Case {
        name: &'static str,
        group: BenchGroup,
        shape: BenchShape,
        note: &'static str,
        seed_start: Option<&'static str>,
        voucher: Option<&'static str>,
        pack: Option<&'static str>,
        tag1: Option<&'static str>,
        tag2: Option<&'static str>,
        joker: Option<&'static str>,
        joker_location: Option<&'static str>,
        souls: f64,
        observatory: bool,
        perkeo: bool,
        deck: Option<&'static str>,
        erratic: bool,
        no_faces: bool,
        min_face_cards: i32,
        suit_ratio: f64,
        num_seeds: i64,
        threads: i32,
    }

    struct Dll {
        handle: HModule,
        entry: DllEntry,
        free_result: FreeResult,
        set_cuda_enabled: Option<SetCudaEnabled>,
    }

    enum DllEntry {
        Current(BrainstormSearch),
        Original(OriginalBrainstorm),
    }

    impl Dll {
        fn load(path: &str) -> Result<Self, String> {
            Self::load_current(path)
        }

        fn load_current(path: &str) -> Result<Self, String> {
            let mut wide: Vec<u16> = OsStr::new(path).encode_wide().collect();
            wide.push(0);
            let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
            if handle.is_null() {
                return Err(format!("failed to load DLL: {path}"));
            }

            let search_name = CString::new("brainstorm_search").map_err(|err| format!("{err}"))?;
            let free_name = CString::new("free_result").map_err(|err| format!("{err}"))?;
            let cuda_name =
                CString::new("immolate_set_cuda_enabled").map_err(|err| format!("{err}"))?;
            let search_ptr = unsafe { GetProcAddress(handle, search_name.as_ptr()) };
            let free_ptr = unsafe { GetProcAddress(handle, free_name.as_ptr()) };
            let cuda_ptr = unsafe { GetProcAddress(handle, cuda_name.as_ptr()) };
            if search_ptr.is_null() || free_ptr.is_null() {
                unsafe {
                    FreeLibrary(handle);
                }
                return Err(format!(
                    "missing required exports in {path}: brainstorm_search/free_result",
                ));
            }

            Ok(Self {
                handle,
                entry: DllEntry::Current(unsafe {
                    std::mem::transmute::<FarProc, BrainstormSearch>(search_ptr)
                }),
                free_result: unsafe { std::mem::transmute::<FarProc, FreeResult>(free_ptr) },
                set_cuda_enabled: (!cuda_ptr.is_null())
                    .then(|| unsafe { std::mem::transmute::<FarProc, SetCudaEnabled>(cuda_ptr) }),
            })
        }

        fn set_cuda_enabled(&self, enabled: bool) {
            if let Some(set_cuda_enabled) = self.set_cuda_enabled {
                unsafe {
                    set_cuda_enabled(enabled);
                }
            }
        }

        fn run(&self, case: &Case) -> Result<Option<String>, String> {
            match self.entry {
                DllEntry::Current(search) => self.run_current(case, search),
                DllEntry::Original(search) => self.run_original(case, search),
            }
        }

        fn load_original(path: &str) -> Result<Self, String> {
            let mut wide: Vec<u16> = OsStr::new(path).encode_wide().collect();
            wide.push(0);
            let _silencer = StdoutSilencer::start();
            let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
            if handle.is_null() {
                return Err(format!("failed to load original DLL: {path}"));
            }

            let search_name = CString::new("brainstorm").map_err(|err| format!("{err}"))?;
            let free_name = CString::new("free_result").map_err(|err| format!("{err}"))?;
            let search_ptr = unsafe { GetProcAddress(handle, search_name.as_ptr()) };
            let free_ptr = unsafe { GetProcAddress(handle, free_name.as_ptr()) };
            if search_ptr.is_null() || free_ptr.is_null() {
                unsafe {
                    FreeLibrary(handle);
                }
                return Err(format!(
                    "missing required exports in {path}: brainstorm/free_result",
                ));
            }

            Ok(Self {
                handle,
                entry: DllEntry::Original(unsafe {
                    std::mem::transmute::<FarProc, OriginalBrainstorm>(search_ptr)
                }),
                free_result: unsafe { std::mem::transmute::<FarProc, FreeResult>(free_ptr) },
                set_cuda_enabled: None,
            })
        }

        fn run_current(
            &self,
            case: &Case,
            search: BrainstormSearch,
        ) -> Result<Option<String>, String> {
            let seed_start = CArg::new(case.seed_start)?;
            let voucher = CArg::new(case.voucher)?;
            let pack = CArg::new(case.pack)?;
            let tag1 = CArg::new(case.tag1)?;
            let tag2 = CArg::new(case.tag2)?;
            let joker = CArg::new(case.joker)?;
            let joker_location = CArg::new(case.joker_location)?;
            let deck = CArg::new(case.deck)?;

            let result = unsafe {
                (search)(
                    seed_start.as_ptr(),
                    voucher.as_ptr(),
                    pack.as_ptr(),
                    tag1.as_ptr(),
                    tag2.as_ptr(),
                    joker.as_ptr(),
                    joker_location.as_ptr(),
                    case.souls,
                    case.observatory,
                    case.perkeo,
                    deck.as_ptr(),
                    case.erratic,
                    case.no_faces,
                    case.min_face_cards,
                    case.suit_ratio,
                    case.num_seeds,
                    case.threads,
                )
            };
            if result.is_null() {
                return Ok(None);
            }
            let out = unsafe { CStr::from_ptr(result) }
                .to_string_lossy()
                .into_owned();
            unsafe {
                (self.free_result)(result);
            }
            Ok(Some(out))
        }

        fn run_original(
            &self,
            case: &Case,
            search: OriginalBrainstorm,
        ) -> Result<Option<String>, String> {
            let seed_start = CArg::new(Some(case.seed_start.unwrap_or("")))?;
            let voucher = CArg::new(Some(original_voucher_name(case.voucher.unwrap_or(""))?))?;
            let pack = CArg::new(Some(original_pack_name(case.pack.unwrap_or(""))?))?;
            let tag = CArg::new(Some(original_tag_name(case.tag1.unwrap_or(""))?))?;

            let _silencer = StdoutSilencer::start();
            let result = unsafe {
                (search)(
                    seed_start.as_ptr(),
                    voucher.as_ptr(),
                    pack.as_ptr(),
                    tag.as_ptr(),
                    case.souls,
                    case.observatory,
                    case.perkeo,
                )
            };
            if result.is_null() {
                return Ok(None);
            }
            let out = unsafe { CStr::from_ptr(result) }
                .to_string_lossy()
                .into_owned();
            unsafe {
                (self.free_result)(result as *mut c_char);
            }
            Ok(Some(out))
        }
    }

    struct StdoutSilencer {
        previous_fd: c_int,
        nul_fd: c_int,
        active: bool,
    }

    impl StdoutSilencer {
        fn start() -> Self {
            unsafe {
                fflush(ptr::null_mut());
            }
            let mut nul_path: Vec<u16> = OsStr::new("NUL").encode_wide().collect();
            nul_path.push(0);
            let previous_fd = unsafe { _dup(STDOUT_FILENO) };
            if previous_fd < 0 {
                return Self {
                    previous_fd,
                    nul_fd: -1,
                    active: false,
                };
            }

            let nul = unsafe {
                CreateFileW(
                    nul_path.as_ptr(),
                    GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    ptr::null_mut(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    ptr::null_mut(),
                )
            };
            if nul.is_null() || nul == INVALID_HANDLE_VALUE {
                unsafe {
                    _close(previous_fd);
                }
                return Self {
                    previous_fd: -1,
                    nul_fd: -1,
                    active: false,
                };
            }

            let nul_fd = unsafe { _open_osfhandle(nul as isize, O_WRONLY) };
            if nul_fd < 0 {
                unsafe {
                    CloseHandle(nul);
                    _close(previous_fd);
                }
                return Self {
                    previous_fd: -1,
                    nul_fd: -1,
                    active: false,
                };
            }

            let active = unsafe { _dup2(nul_fd, STDOUT_FILENO) == 0 };
            if !active {
                unsafe {
                    _dup2(previous_fd, STDOUT_FILENO);
                    _close(previous_fd);
                    _close(nul_fd);
                }
                return Self {
                    previous_fd: -1,
                    nul_fd: -1,
                    active: false,
                };
            }

            Self {
                previous_fd,
                nul_fd,
                active,
            }
        }
    }

    impl Drop for StdoutSilencer {
        fn drop(&mut self) {
            if self.active {
                unsafe {
                    fflush(ptr::null_mut());
                    _dup2(self.previous_fd, STDOUT_FILENO);
                    _close(self.previous_fd);
                    _close(self.nul_fd);
                }
                self.previous_fd = -1;
                self.nul_fd = -1;
            }
        }
    }

    impl Drop for Dll {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    FreeLibrary(self.handle);
                }
            }
        }
    }

    struct CArg {
        value: Option<CString>,
    }

    impl CArg {
        fn new(value: Option<&str>) -> Result<Self, String> {
            value
                .map(|value| CString::new(value).map_err(|err| format!("{err}")))
                .transpose()
                .map(|value| Self { value })
        }

        fn as_ptr(&self) -> *const c_char {
            self.value
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr())
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum OutputFormat {
        Pretty,
        Tsv,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ColorMode {
        Auto,
        Always,
        Never,
    }

    #[derive(Clone, Copy, Debug)]
    struct OutputOptions {
        format: OutputFormat,
        color: ColorMode,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum CudaBenchMode {
        Default,
        Off,
        On,
        Both,
    }

    impl CudaBenchMode {
        fn measurements(self, default_label: &'static str) -> Vec<(&'static str, Option<bool>)> {
            match self {
                Self::Default => vec![(default_label, None)],
                Self::Off => vec![("rust-cpu", Some(false))],
                Self::On => vec![("rust-cuda", Some(true))],
                Self::Both => vec![("rust-cpu", Some(false)), ("rust-cuda", Some(true))],
            }
        }
    }

    impl Default for OutputOptions {
        fn default() -> Self {
            Self {
                format: OutputFormat::Pretty,
                color: ColorMode::Auto,
            }
        }
    }

    impl OutputOptions {
        fn use_color(self) -> bool {
            match self.color {
                ColorMode::Always => true,
                ColorMode::Never => false,
                ColorMode::Auto => io::stdout().is_terminal(),
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct BenchSettings<'a> {
        selected_case: &'a str,
        budget: i64,
        threads: i32,
        repeat: usize,
        warmup: usize,
        output: OutputOptions,
    }

    enum Command {
        Bench {
            dll: String,
            case: String,
            budget: i64,
            threads: i32,
            repeat: usize,
            warmup: usize,
            cuda: CudaBenchMode,
            output: OutputOptions,
        },
        BenchCompare {
            rust: String,
            original: String,
            case: String,
            budget: i64,
            threads: i32,
            repeat: usize,
            warmup: usize,
            min_ratio: f64,
            fail_on_mismatch: bool,
            cuda: CudaBenchMode,
            output: OutputOptions,
        },
    }

    pub fn main() {
        match parse_command(env::args().skip(1).collect()) {
            Ok(Command::Bench {
                dll,
                case,
                budget,
                threads,
                repeat,
                warmup,
                cuda,
                output,
            }) => {
                let settings = BenchSettings {
                    selected_case: &case,
                    budget,
                    threads,
                    repeat,
                    warmup,
                    output,
                };
                if let Err(err) = bench(&dll, settings, cuda) {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            },
            Ok(Command::BenchCompare {
                rust,
                original,
                case,
                budget,
                threads,
                repeat,
                warmup,
                min_ratio,
                fail_on_mismatch,
                cuda,
                output,
            }) => {
                let settings = BenchSettings {
                    selected_case: &case,
                    budget,
                    threads,
                    repeat,
                    warmup,
                    output,
                };
                if let Err(err) = bench_compare(
                    &rust,
                    &original,
                    settings,
                    min_ratio,
                    fail_on_mismatch,
                    cuda,
                ) {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            },
            Err(err) => {
                eprintln!("{err}");
                eprintln!(
                    "usage:\n  immolate_dll_harness bench --dll PATH [--case all|cuda-long|GROUP|NAME] [--budget N] [--threads N] [--repeat N] [--warmup N] [--cuda default|off|on|both] [--format pretty|tsv] [--color auto|always|never]\n  immolate_dll_harness bench-compare --rust PATH --original PATH [--case all|cuda-long|GROUP|NAME] [--budget N] [--threads N] [--repeat N] [--warmup N] [--cuda default|off|on|both] [--min-ratio N] [--fail-on-mismatch true|false] [--format pretty|tsv] [--color auto|always|never]"
                );
                std::process::exit(2);
            },
        }
    }

    fn bench(
        dll_path: &str,
        settings: BenchSettings<'_>,
        cuda: CudaBenchMode,
    ) -> Result<(), String> {
        if settings.budget <= 0 {
            return Err("--budget must be positive".to_owned());
        }
        if settings.repeat == 0 {
            return Err("--repeat must be positive".to_owned());
        }
        let dll = Dll::load(dll_path)?;
        let cases =
            selected_bench_cases(settings.selected_case, settings.budget, settings.threads)?;

        if settings.output.format == OutputFormat::Tsv {
            print_tsv_header();
        } else {
            print_run_header("Brainstorm DLL Benchmark", settings, cases.len());
        }

        let measurements = cuda.measurements("dll");
        let mut failed = false;
        let mut summaries = Vec::with_capacity(cases.len() * measurements.len());
        for case in &cases {
            let mut cpu_result = None::<String>;
            for (implementation, cuda_enabled) in &measurements {
                let summary = measure_bench_case(
                    &dll,
                    case,
                    settings.repeat,
                    settings.warmup,
                    implementation,
                    settings.output,
                    *cuda_enabled,
                )?;
                match *implementation {
                    "rust-cpu" => cpu_result = Some(summary.result.clone()),
                    "rust-cuda" => {
                        if cpu_result
                            .as_ref()
                            .is_some_and(|cpu| *cpu != summary.result)
                        {
                            failed = true;
                            eprintln!(
                                "CUDA parity mismatch in {}: rust-cpu={} rust-cuda={}",
                                summary.case_name,
                                cpu_result.as_deref().unwrap_or("<missing>"),
                                summary.result
                            );
                        }
                    },
                    _ => {},
                }
                summaries.push(summary);
            }
        }
        if settings.output.format == OutputFormat::Pretty {
            print_single_bench_report(&summaries, settings.output);
        }
        if failed {
            Err("CUDA parity failed".to_owned())
        } else {
            Ok(())
        }
    }

    fn bench_compare(
        rust_path: &str,
        original_path: &str,
        settings: BenchSettings<'_>,
        min_ratio: f64,
        fail_on_mismatch: bool,
        cuda: CudaBenchMode,
    ) -> Result<(), String> {
        if settings.budget <= 0 {
            return Err("--budget must be positive".to_owned());
        }
        if settings.repeat == 0 {
            return Err("--repeat must be positive".to_owned());
        }
        if min_ratio < 0.0 {
            return Err("--min-ratio cannot be negative".to_owned());
        }
        let rust = Dll::load(rust_path)?;
        let original = Dll::load_original(original_path)?;
        let cases =
            selected_bench_cases(settings.selected_case, settings.budget, settings.threads)?;
        if settings.output.format == OutputFormat::Tsv {
            print_tsv_header();
        } else {
            print_run_header(
                "Brainstorm DLL Benchmark: Rust CPU/CUDA vs Original",
                settings,
                cases.len(),
            );
        }

        let mut failed = false;
        let mut comparisons = Vec::with_capacity(cases.len());
        for case in &cases {
            let (rust_summary, rust_cuda_summary) = match cuda {
                CudaBenchMode::Off => (
                    measure_bench_case(
                        &rust,
                        case,
                        settings.repeat,
                        settings.warmup,
                        "rust-cpu",
                        settings.output,
                        Some(false),
                    )?,
                    None,
                ),
                CudaBenchMode::Default => (
                    measure_bench_case(
                        &rust,
                        case,
                        settings.repeat,
                        settings.warmup,
                        "rust",
                        settings.output,
                        None,
                    )?,
                    None,
                ),
                CudaBenchMode::On | CudaBenchMode::Both => {
                    let rust_cpu = measure_bench_case(
                        &rust,
                        case,
                        settings.repeat,
                        settings.warmup,
                        "rust-cpu",
                        settings.output,
                        Some(false),
                    )?;
                    let rust_cuda = Some(measure_bench_case(
                        &rust,
                        case,
                        settings.repeat,
                        settings.warmup,
                        "rust-cuda",
                        settings.output,
                        Some(true),
                    )?);
                    (rust_cpu, rust_cuda)
                },
            };
            let (original_summary, original_skip) = match original_skip_reason(case) {
                Some(reason) => (None, Some(reason)),
                None => (
                    Some(measure_bench_case(
                        &original,
                        case,
                        settings.repeat,
                        settings.warmup,
                        "original",
                        settings.output,
                        None,
                    )?),
                    None,
                ),
            };
            let comparison = BenchComparison {
                rust: rust_summary,
                rust_cuda: rust_cuda_summary,
                original: original_summary,
                original_skip,
            };
            if min_ratio > 0.0
                && comparison
                    .rust_vs_original_ratio()
                    .is_some_and(|ratio| ratio < min_ratio)
            {
                failed = true;
            }
            if min_ratio > 0.0
                && comparison
                    .cuda_vs_original_ratio()
                    .is_some_and(|ratio| ratio < min_ratio)
            {
                failed = true;
            }
            if comparison.has_cuda_result_mismatch() {
                failed = true;
                let rust_cuda = comparison
                    .rust_cuda
                    .as_ref()
                    .expect("CUDA mismatch requires CUDA result");
                eprintln!(
                    "CUDA parity mismatch in {}: rust-cpu={} rust-cuda={}",
                    comparison.rust.case_name, comparison.rust.result, rust_cuda.result
                );
            }
            if let Some(original) = comparison.original.as_ref() {
                if fail_on_mismatch && comparison.rust.result != original.result {
                    failed = true;
                    eprintln!(
                        "benchmark parity mismatch in {}: rust={} original={}",
                        comparison.rust.case_name, comparison.rust.result, original.result
                    );
                }
            }
            if settings.output.format == OutputFormat::Tsv {
                print_tsv_compare(&comparison, min_ratio);
            }
            comparisons.push(comparison);
        }
        if settings.output.format == OutputFormat::Pretty {
            print_compare_report(&comparisons, min_ratio, settings.output);
        }
        if failed {
            Err("benchmark threshold or parity failed".to_owned())
        } else {
            Ok(())
        }
    }

    struct BenchRun {
        run: usize,
        elapsed: Duration,
        scanned: i64,
        seeds_per_sec: f64,
        ns_per_seed: f64,
        result: String,
    }

    struct BenchSummary {
        implementation: &'static str,
        case_name: &'static str,
        group: BenchGroup,
        shape: BenchShape,
        note: &'static str,
        budget: i64,
        threads: i32,
        repeat: usize,
        mean_elapsed: Duration,
        min_elapsed: Duration,
        max_elapsed: Duration,
        p50_elapsed: Duration,
        p95_elapsed: Duration,
        p99_elapsed: Duration,
        stdev_elapsed: Duration,
        coefficient_variation: f64,
        mean_scanned: f64,
        scanned_pct: f64,
        seeds_per_sec: f64,
        ns_per_seed: f64,
        result: String,
    }

    struct BenchComparison {
        rust: BenchSummary,
        rust_cuda: Option<BenchSummary>,
        original: Option<BenchSummary>,
        original_skip: Option<&'static str>,
    }

    impl BenchComparison {
        fn rust_vs_original_ratio(&self) -> Option<f64> {
            self.original.as_ref().map(|original| {
                original.mean_elapsed.as_secs_f64() / self.rust.mean_elapsed.as_secs_f64()
            })
        }

        fn cuda_vs_original_ratio(&self) -> Option<f64> {
            self.original.as_ref().and_then(|original| {
                self.rust_cuda.as_ref().map(|rust_cuda| {
                    original.mean_elapsed.as_secs_f64() / rust_cuda.mean_elapsed.as_secs_f64()
                })
            })
        }

        fn cuda_vs_cpu_ratio(&self) -> Option<f64> {
            self.rust_cuda.as_ref().map(|rust_cuda| {
                self.rust.mean_elapsed.as_secs_f64() / rust_cuda.mean_elapsed.as_secs_f64()
            })
        }

        fn has_result_mismatch(&self) -> bool {
            self.original
                .as_ref()
                .is_some_and(|original| self.rust.result != original.result)
        }

        fn has_cuda_result_mismatch(&self) -> bool {
            self.rust_cuda
                .as_ref()
                .is_some_and(|rust_cuda| self.rust.result != rust_cuda.result)
        }
    }

    fn measure_bench_case(
        dll: &Dll,
        case: &Case,
        repeat: usize,
        warmup: usize,
        implementation: &'static str,
        output: OutputOptions,
        cuda_enabled: Option<bool>,
    ) -> Result<BenchSummary, String> {
        if let Some(enabled) = cuda_enabled {
            dll.set_cuda_enabled(enabled);
        }
        run_warmups(dll, case, warmup)?;
        let mut runs = Vec::with_capacity(repeat);
        let mut scanned_counts = Vec::with_capacity(repeat);
        for run in 1..=repeat {
            let started = Instant::now();
            let result = dll.run(case);
            let elapsed = started.elapsed();
            let mut result = result?;
            if implementation == "original" {
                result = normalize_legacy_original_result(case, result);
            }
            let scanned = scanned_count(case, result.as_deref());
            let elapsed_secs = elapsed.as_secs_f64();
            let seeds_per_sec = scanned as f64 / elapsed_secs;
            let ns_per_seed = elapsed_secs * 1_000_000_000.0 / scanned as f64;
            scanned_counts.push(scanned);
            let bench_run = BenchRun {
                run,
                elapsed,
                scanned,
                seeds_per_sec,
                ns_per_seed,
                result: display_result(result.as_deref()).to_owned(),
            };
            if output.format == OutputFormat::Tsv {
                print_tsv_run(implementation, case, &bench_run);
            }
            runs.push(bench_run);
        }
        let mut durations: Vec<_> = runs.iter().map(|run| run.elapsed).collect();
        durations.sort_by(compare_duration);
        let mean_elapsed = mean_duration(&durations);
        let min_elapsed = durations[0];
        let max_elapsed = durations[durations.len() - 1];
        let p50_elapsed = percentile(&durations, 0.50);
        let p95_elapsed = percentile(&durations, 0.95);
        let p99_elapsed = percentile(&durations, 0.99);
        let stdev_elapsed = stdev_duration(&durations, mean_elapsed);
        let coefficient_variation = stdev_elapsed.as_secs_f64() / mean_elapsed.as_secs_f64();
        let mean_scanned = scanned_counts
            .iter()
            .map(|value| *value as f64)
            .sum::<f64>()
            / repeat as f64;
        let seeds_per_sec = mean_scanned / mean_elapsed.as_secs_f64();
        let ns_per_seed = mean_elapsed.as_secs_f64() * 1_000_000_000.0 / mean_scanned;
        let scanned_pct = mean_scanned / case.num_seeds as f64;
        let result = runs
            .last()
            .map_or_else(|| "<none>".to_owned(), |run| run.result.clone());
        let summary = BenchSummary {
            implementation,
            case_name: case.name,
            group: case.group,
            shape: case.shape,
            note: case.note,
            budget: case.num_seeds,
            threads: case.threads,
            repeat,
            mean_elapsed,
            min_elapsed,
            max_elapsed,
            p50_elapsed,
            p95_elapsed,
            p99_elapsed,
            stdev_elapsed,
            coefficient_variation,
            mean_scanned,
            scanned_pct,
            seeds_per_sec,
            ns_per_seed,
            result,
        };
        if output.format == OutputFormat::Tsv {
            print_tsv_summary(&summary);
        }
        Ok(summary)
    }

    fn run_warmups(dll: &Dll, case: &Case, warmup: usize) -> Result<(), String> {
        for _ in 0..warmup {
            dll.run(case)?;
        }
        Ok(())
    }

    fn parse_command(args: Vec<String>) -> Result<Command, String> {
        let Some(mode) = args.first() else {
            return Err("missing command".to_owned());
        };
        match mode.as_str() {
            "bench" => {
                let mut dll = None;
                let mut case = "all".to_owned();
                let mut budget = 1_000_000;
                let mut threads = 0;
                let mut repeat = 5;
                let mut warmup = 1;
                let mut cuda = CudaBenchMode::Default;
                let mut output = OutputOptions::default();
                parse_flags(&args[1..], |flag, value| match flag {
                    "--dll" => {
                        dll = Some(value.to_owned());
                        Ok(())
                    },
                    "--case" => {
                        value.clone_into(&mut case);
                        Ok(())
                    },
                    "--budget" => {
                        budget = parse_value(value, "--budget")?;
                        Ok(())
                    },
                    "--threads" => {
                        threads = parse_value(value, "--threads")?;
                        Ok(())
                    },
                    "--repeat" => {
                        repeat = parse_value(value, "--repeat")?;
                        Ok(())
                    },
                    "--warmup" => {
                        warmup = parse_value(value, "--warmup")?;
                        Ok(())
                    },
                    "--cuda" => {
                        cuda = parse_cuda_bench_mode(value)?;
                        Ok(())
                    },
                    "--format" => {
                        output.format = parse_output_format(value)?;
                        Ok(())
                    },
                    "--color" => {
                        output.color = parse_color_mode(value)?;
                        Ok(())
                    },
                    _ => Err(format!("unknown bench flag: {flag}")),
                })?;
                Ok(Command::Bench {
                    dll: dll.ok_or_else(|| "missing --dll".to_owned())?,
                    case,
                    budget,
                    threads,
                    repeat,
                    warmup,
                    cuda,
                    output,
                })
            },
            "bench-compare" => {
                let mut rust = None;
                let mut original = None;
                let mut case = "all".to_owned();
                let mut budget = 1_000_000;
                let mut threads = 0;
                let mut repeat = 5;
                let mut warmup = 1;
                let mut min_ratio = 1.0;
                let mut fail_on_mismatch = false;
                let mut cuda = CudaBenchMode::Both;
                let mut output = OutputOptions::default();
                parse_flags(&args[1..], |flag, value| match flag {
                    "--rust" => {
                        rust = Some(value.to_owned());
                        Ok(())
                    },
                    "--original" => {
                        original = Some(value.to_owned());
                        Ok(())
                    },
                    "--case" => {
                        value.clone_into(&mut case);
                        Ok(())
                    },
                    "--budget" => {
                        budget = parse_value(value, "--budget")?;
                        Ok(())
                    },
                    "--threads" => {
                        threads = parse_value(value, "--threads")?;
                        Ok(())
                    },
                    "--repeat" => {
                        repeat = parse_value(value, "--repeat")?;
                        Ok(())
                    },
                    "--warmup" => {
                        warmup = parse_value(value, "--warmup")?;
                        Ok(())
                    },
                    "--min-ratio" => {
                        min_ratio = parse_value(value, "--min-ratio")?;
                        Ok(())
                    },
                    "--fail-on-mismatch" => {
                        fail_on_mismatch = parse_bool_flag(value, "--fail-on-mismatch")?;
                        Ok(())
                    },
                    "--cuda" => {
                        cuda = parse_cuda_bench_mode(value)?;
                        Ok(())
                    },
                    "--format" => {
                        output.format = parse_output_format(value)?;
                        Ok(())
                    },
                    "--color" => {
                        output.color = parse_color_mode(value)?;
                        Ok(())
                    },
                    _ => Err(format!("unknown bench-compare flag: {flag}")),
                })?;
                Ok(Command::BenchCompare {
                    rust: rust.ok_or_else(|| "missing --rust".to_owned())?,
                    original: original.ok_or_else(|| "missing --original".to_owned())?,
                    case,
                    budget,
                    threads,
                    repeat,
                    warmup,
                    min_ratio,
                    fail_on_mismatch,
                    cuda,
                    output,
                })
            },
            _ => Err(format!("unknown command: {mode}")),
        }
    }

    fn parse_flags<F>(args: &[String], mut visit: F) -> Result<(), String>
    where
        F: FnMut(&str, &str) -> Result<(), String>,
    {
        let mut idx = 0;
        while idx < args.len() {
            let flag = &args[idx];
            let value = args
                .get(idx + 1)
                .ok_or_else(|| format!("missing value for {flag}"))?;
            visit(flag, value)?;
            idx += 2;
        }
        Ok(())
    }

    fn parse_value<T>(value: &str, flag: &str) -> Result<T, String>
    where
        T: std::str::FromStr,
    {
        value
            .parse::<T>()
            .map_err(|_| format!("invalid {flag}: {value}"))
    }

    fn parse_output_format(value: &str) -> Result<OutputFormat, String> {
        match value {
            "pretty" => Ok(OutputFormat::Pretty),
            "tsv" => Ok(OutputFormat::Tsv),
            _ => Err(format!("invalid --format: {value}")),
        }
    }

    fn parse_color_mode(value: &str) -> Result<ColorMode, String> {
        match value {
            "auto" => Ok(ColorMode::Auto),
            "always" => Ok(ColorMode::Always),
            "never" => Ok(ColorMode::Never),
            _ => Err(format!("invalid --color: {value}")),
        }
    }

    fn parse_bool_flag(value: &str, flag: &str) -> Result<bool, String> {
        match value {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err(format!("invalid {flag}: {value}")),
        }
    }

    fn parse_cuda_bench_mode(value: &str) -> Result<CudaBenchMode, String> {
        match value {
            "default" => Ok(CudaBenchMode::Default),
            "off" | "cpu" => Ok(CudaBenchMode::Off),
            "on" | "cuda" => Ok(CudaBenchMode::On),
            "both" => Ok(CudaBenchMode::Both),
            _ => Err(format!("invalid --cuda: {value}")),
        }
    }

    fn display_result(result: Option<&str>) -> &str {
        result.unwrap_or("<null>")
    }

    fn normalize_legacy_original_result(case: &Case, result: Option<String>) -> Option<String> {
        result.filter(|seed| result_within_budget(case, seed))
    }

    fn result_within_budget(case: &Case, result: &str) -> bool {
        result.is_empty() || seed_scan_count(case, result) <= case.num_seeds
    }

    fn scanned_count(case: &Case, result: Option<&str>) -> i64 {
        let Some(result) = result else {
            return case.num_seeds;
        };
        if result.is_empty() {
            return 1;
        }
        seed_scan_count(case, result).min(case.num_seeds)
    }

    fn seed_scan_count(case: &Case, result: &str) -> i64 {
        let start = case.seed_start.unwrap_or("");
        (Seed::from_str(result).id() - Seed::from_str(start).id()).rem_euclid(SEED_SPACE) + 1
    }

    fn original_skip_reason(case: &Case) -> Option<&'static str> {
        if case.shape == BenchShape::Miss {
            return Some("legacy DLL has a fixed 100M scan cap, so miss cases are unbounded");
        }
        if !case.tag2.unwrap_or("").is_empty() {
            return Some("legacy DLL supports only one tag filter");
        }
        if !case.joker.unwrap_or("").is_empty() {
            return Some("legacy DLL has no joker filter");
        }
        if !matches!(case.deck.unwrap_or(""), "" | "b_red") {
            return Some("legacy DLL has no deck filter");
        }
        if case.erratic
            || case.no_faces
            || case.min_face_cards != 0
            || case.suit_ratio.abs() > f64::EPSILON
        {
            return Some("legacy DLL has no Erratic Deck filters");
        }
        if original_voucher_name(case.voucher.unwrap_or("")).is_err()
            || original_pack_name(case.pack.unwrap_or("")).is_err()
            || original_tag_name(case.tag1.unwrap_or("")).is_err()
        {
            return Some("legacy DLL does not recognize one of this case's filters");
        }
        None
    }

    fn original_voucher_name(key: &str) -> Result<&'static str, String> {
        match key {
            "" => Ok(""),
            "v_overstock_norm" => Ok("Overstock"),
            "v_overstock_plus" => Ok("Overstock Plus"),
            "v_clearance_sale" => Ok("Clearance Sale"),
            "v_liquidation" => Ok("Liquidation"),
            "v_hone" => Ok("Hone"),
            "v_glow_up" => Ok("Glow Up"),
            "v_reroll_surplus" => Ok("Reroll Surplus"),
            "v_reroll_glut" => Ok("Reroll Glut"),
            "v_crystal_ball" => Ok("Crystal Ball"),
            "v_omen_globe" => Ok("Omen Globe"),
            "v_telescope" => Ok("Telescope"),
            "v_observatory" => Ok("Observatory"),
            "v_grabber" => Ok("Grabber"),
            "v_nacho_tong" => Ok("Nacho Tong"),
            "v_wasteful" => Ok("Wasteful"),
            "v_recyclomancy" => Ok("Recyclomancy"),
            "v_tarot_merchant" => Ok("Tarot Merchant"),
            "v_tarot_tycoon" => Ok("Tarot Tycoon"),
            "v_planet_merchant" => Ok("Planet Merchant"),
            "v_planet_tycoon" => Ok("Planet Tycoon"),
            "v_seed_money" => Ok("Seed Money"),
            "v_money_tree" => Ok("Money Tree"),
            "v_blank" => Ok("Blank"),
            "v_antimatter" => Ok("Antimatter"),
            "v_magic_trick" => Ok("Magic Trick"),
            "v_illusion" => Ok("Illusion"),
            "v_hieroglyph" => Ok("Hieroglyph"),
            "v_petroglyph" => Ok("Petroglyph"),
            "v_directors_cut" => Ok("Director's Cut"),
            "v_retcon" => Ok("Retcon"),
            "v_paint_brush" => Ok("Paint Brush"),
            "v_palette" => Ok("Palette"),
            _ => Err(format!("unsupported original voucher key: {key}")),
        }
    }

    fn original_pack_name(key: &str) -> Result<&'static str, String> {
        match normalize_original_pack_key(key).as_str() {
            "" => Ok(""),
            "p_arcana_normal" => Ok("Arcana Pack"),
            "p_arcana_jumbo" => Ok("Jumbo Arcana Pack"),
            "p_arcana_mega" => Ok("Mega Arcana Pack"),
            "p_celestial_normal" => Ok("Celestial Pack"),
            "p_celestial_jumbo" => Ok("Jumbo Celestial Pack"),
            "p_celestial_mega" => Ok("Mega Celestial Pack"),
            "p_standard_normal" => Ok("Standard Pack"),
            "p_standard_jumbo" => Ok("Jumbo Standard Pack"),
            "p_standard_mega" => Ok("Mega Standard Pack"),
            "p_buffoon_normal" => Ok("Buffoon Pack"),
            "p_buffoon_jumbo" => Ok("Jumbo Buffoon Pack"),
            "p_buffoon_mega" => Ok("Mega Buffoon Pack"),
            "p_spectral_normal" => Ok("Spectral Pack"),
            "p_spectral_jumbo" => Ok("Jumbo Spectral Pack"),
            "p_spectral_mega" => Ok("Mega Spectral Pack"),
            _ => Err(format!("unsupported original pack key: {key}")),
        }
    }

    fn normalize_original_pack_key(key: &str) -> String {
        let Some((prefix, suffix)) = key.rsplit_once('_') else {
            return key.to_owned();
        };
        if suffix.chars().all(|ch| ch.is_ascii_digit()) {
            prefix.to_owned()
        } else {
            key.to_owned()
        }
    }

    fn original_tag_name(key: &str) -> Result<&'static str, String> {
        match key {
            "" => Ok(""),
            "tag_uncommon" => Ok("Uncommon Tag"),
            "tag_rare" => Ok("Rare Tag"),
            "tag_negative" => Ok("Negative Tag"),
            "tag_foil" => Ok("Foil Tag"),
            "tag_holo" => Ok("Holographic Tag"),
            "tag_polychrome" => Ok("Polychrome Tag"),
            "tag_investment" => Ok("Investment Tag"),
            "tag_voucher" => Ok("Voucher Tag"),
            "tag_boss" => Ok("Boss Tag"),
            "tag_standard" => Ok("Standard Tag"),
            "tag_charm" => Ok("Charm Tag"),
            "tag_meteor" => Ok("Meteor Tag"),
            "tag_buffoon" => Ok("Buffoon Tag"),
            "tag_handy" => Ok("Handy Tag"),
            "tag_garbage" => Ok("Garbage Tag"),
            "tag_ethereal" => Ok("Ethereal Tag"),
            "tag_coupon" => Ok("Coupon Tag"),
            "tag_double" => Ok("Double Tag"),
            "tag_juggle" => Ok("Juggle Tag"),
            "tag_d_six" => Ok("D6 Tag"),
            "tag_top_up" => Ok("Top-up Tag"),
            "tag_skip" => Ok("Speed Tag"),
            "tag_orbital" => Ok("Orbital Tag"),
            "tag_economy" => Ok("Economy Tag"),
            _ => Err(format!("unsupported original tag key: {key}")),
        }
    }

    fn compare_duration(a: &Duration, b: &Duration) -> CmpOrdering {
        a.partial_cmp(b).unwrap_or(CmpOrdering::Equal)
    }

    fn percentile(values: &[Duration], pct: f64) -> Duration {
        let idx = ((values.len().saturating_sub(1)) as f64 * pct).ceil() as usize;
        values[idx.min(values.len() - 1)]
    }

    fn mean_duration(values: &[Duration]) -> Duration {
        let total = values.iter().map(Duration::as_secs_f64).sum::<f64>();
        Duration::from_secs_f64(total / values.len() as f64)
    }

    fn stdev_duration(values: &[Duration], mean: Duration) -> Duration {
        let mean_secs = mean.as_secs_f64();
        let variance = values
            .iter()
            .map(|value| {
                let delta = value.as_secs_f64() - mean_secs;
                delta * delta
            })
            .sum::<f64>()
            / values.len() as f64;
        Duration::from_secs_f64(variance.sqrt())
    }

    const ANSI_RESET: &str = "\x1b[0m";
    const ANSI_DIM: &str = "\x1b[2m";
    const ANSI_RED: &str = "\x1b[31m";
    const ANSI_GREEN: &str = "\x1b[32m";
    const ANSI_YELLOW: &str = "\x1b[33m";

    fn paint(enabled: bool, code: &str, text: &str) -> String {
        if enabled {
            format!("{code}{text}{ANSI_RESET}")
        } else {
            text.to_owned()
        }
    }

    fn print_run_header(title: &str, settings: BenchSettings<'_>, case_count: usize) {
        let color = settings.output.use_color();
        println!("{title}");
        println!(
            "case={} budget={} repeat={} warmup={} threads={} cases={}",
            settings.selected_case,
            format_integer(settings.budget),
            settings.repeat,
            settings.warmup,
            settings.threads,
            case_count,
        );
        println!(
            "  groups: {}",
            paint(color, ANSI_DIM, &bench::bench_group_keys().join(", "))
        );
        println!();
    }

    fn print_single_bench_report(summaries: &[BenchSummary], output: OutputOptions) {
        let color = output.use_color();
        print_section("Case Summary", color);
        println!(
            "{:<18} {:<9} {:<6} {:>7} {:>11} {:>9} {:>9} {:>10} {:>7} {:<12}",
            "case",
            "group",
            "shape",
            "scan",
            "seeds/s",
            "mean ms",
            "p95 ms",
            "ns/seed",
            "cv",
            "result",
        );
        println!("{}", paint(color, ANSI_DIM, &"-".repeat(111)));
        for summary in summaries {
            let cv = format!("{:.1}%", summary.coefficient_variation * 100.0);
            let cv = paint(
                color,
                cv_color(summary.coefficient_variation),
                &format!("{cv:>7}"),
            );
            println!(
                "{:<18} {:<9} {:<6} {:>7} {:>11} {:>9.3} {:>9.3} {:>10} {} {:<12}",
                summary.case_name,
                summary.group.label(),
                summary.shape.label(),
                format!("{:.1}%", summary.scanned_pct * 100.0),
                format_rate(summary.seeds_per_sec),
                ms(summary.mean_elapsed),
                ms(summary.p95_elapsed),
                format_ns(summary.ns_per_seed),
                cv,
                short_result(&summary.result, 12),
            );
        }
    }

    fn print_compare_report(
        comparisons: &[BenchComparison],
        min_ratio: f64,
        output: OutputOptions,
    ) {
        let color = output.use_color();
        print_section("Rust CPU/CUDA vs Original Brainstorm", color);
        println!(
            "{:<18} {:<9} {:<6} {:>7} {:>11} {:>11} {:>11} {:>10} {:>10} {:>11} {:>17}",
            "case",
            "group",
            "shape",
            "scan",
            "rust/s",
            "cuda/s",
            "original/s",
            "cuda/rust",
            "rust/orig",
            "cuda/orig",
            "mean ms R/G/O",
        );
        println!("{}", paint(color, ANSI_DIM, &"-".repeat(139)));
        for comparison in comparisons {
            if let Some(original) = &comparison.original {
                let rust_ratio = comparison
                    .rust_vs_original_ratio()
                    .expect("original comparison has ratio");
                let rust_ratio_text = paint(
                    color,
                    ratio_color(rust_ratio, min_ratio.max(1.0)),
                    &format!("{rust_ratio:>9.3}x"),
                );
                let cuda_rate = comparison.rust_cuda.as_ref().map_or_else(
                    || "n/a".to_owned(),
                    |summary| format_rate(summary.seeds_per_sec),
                );
                let cuda_cpu_ratio = comparison.cuda_vs_cpu_ratio().map_or_else(
                    || "n/a".to_owned(),
                    |ratio| paint(color, ratio_color(ratio, 1.0), &format!("{ratio:>9.3}x")),
                );
                let cuda_original_ratio = comparison.cuda_vs_original_ratio().map_or_else(
                    || "n/a".to_owned(),
                    |ratio| {
                        paint(
                            color,
                            ratio_color(ratio, min_ratio.max(1.0)),
                            &format!("{ratio:>10.3}x"),
                        )
                    },
                );
                let mean_ms = comparison.rust_cuda.as_ref().map_or_else(
                    || {
                        format!(
                            "{:.3}/n/a/{:.3}",
                            ms(comparison.rust.mean_elapsed),
                            ms(original.mean_elapsed)
                        )
                    },
                    |rust_cuda| {
                        format!(
                            "{:.3}/{:.3}/{:.3}",
                            ms(comparison.rust.mean_elapsed),
                            ms(rust_cuda.mean_elapsed),
                            ms(original.mean_elapsed)
                        )
                    },
                );
                println!(
                    "{:<18} {:<9} {:<6} {:>7} {:>11} {:>11} {:>11} {} {} {} {:>17}",
                    comparison.rust.case_name,
                    comparison.rust.group.label(),
                    comparison.rust.shape.label(),
                    format!("{:.1}%", comparison.rust.scanned_pct * 100.0),
                    format_rate(comparison.rust.seeds_per_sec),
                    cuda_rate,
                    format_rate(original.seeds_per_sec),
                    cuda_cpu_ratio,
                    rust_ratio_text,
                    cuda_original_ratio,
                    mean_ms,
                );
            } else if let Some(reason) = comparison.original_skip {
                let cuda_rate = comparison.rust_cuda.as_ref().map_or_else(
                    || "n/a".to_owned(),
                    |summary| format_rate(summary.seeds_per_sec),
                );
                let cuda_cpu_ratio = comparison
                    .cuda_vs_cpu_ratio()
                    .map_or_else(|| "n/a".to_owned(), |ratio| format!("{ratio:>9.3}x"));
                let mean_ms = comparison.rust_cuda.as_ref().map_or_else(
                    || format!("{:.3}/n/a/n/a", ms(comparison.rust.mean_elapsed)),
                    |rust_cuda| {
                        format!(
                            "{:.3}/{:.3}/n/a",
                            ms(comparison.rust.mean_elapsed),
                            ms(rust_cuda.mean_elapsed)
                        )
                    },
                );
                println!(
                    "{:<18} {:<9} {:<6} {:>7} {:>11} {:>11} {:>11} {:>10} {:>10} {:>11} {:>17} {}",
                    comparison.rust.case_name,
                    comparison.rust.group.label(),
                    comparison.rust.shape.label(),
                    format!("{:.1}%", comparison.rust.scanned_pct * 100.0),
                    format_rate(comparison.rust.seeds_per_sec),
                    cuda_rate,
                    "skipped",
                    cuda_cpu_ratio,
                    "n/a",
                    "n/a",
                    mean_ms,
                    paint(color, ANSI_DIM, reason),
                );
            }
        }
        print_cuda_mismatch_report(comparisons, color);
        print_result_mismatch_report(comparisons, color);
        print_group_report(comparisons, min_ratio, color);
        print_regression_report(comparisons, min_ratio, color);
        print_noise_report(comparisons, color);
    }

    fn print_cuda_mismatch_report(comparisons: &[BenchComparison], color: bool) {
        let mismatches: Vec<_> = comparisons
            .iter()
            .filter(|comparison| comparison.has_cuda_result_mismatch())
            .collect();
        if mismatches.is_empty() {
            return;
        }

        print_section("CUDA Result Mismatches", color);
        println!(
            "  {}",
            paint(
                color,
                ANSI_RED,
                "failing: rust-cuda must match rust-cpu because Rust CPU is the oracle"
            )
        );
        for comparison in mismatches.iter().take(12) {
            let rust_cuda = comparison
                .rust_cuda
                .as_ref()
                .expect("mismatch requires CUDA result");
            println!(
                "  {:<18} rust-cpu {:<12} rust-cuda {}",
                comparison.rust.case_name,
                short_result(&comparison.rust.result, 12),
                short_result(&rust_cuda.result, 12),
            );
        }
        if mismatches.len() > 12 {
            println!("  ... {} more", mismatches.len() - 12);
        }
    }

    fn print_result_mismatch_report(comparisons: &[BenchComparison], color: bool) {
        let mismatches: Vec<_> = comparisons
            .iter()
            .filter(|comparison| comparison.has_result_mismatch())
            .collect();
        if mismatches.is_empty() {
            return;
        }

        print_section("Result Mismatches", color);
        println!(
            "  {}",
            paint(
                color,
                ANSI_DIM,
                "informational: the Original DLL is a historical performance baseline, not the current correctness oracle"
            )
        );
        for comparison in mismatches.iter().take(12) {
            let original = comparison
                .original
                .as_ref()
                .expect("mismatch requires original result");
            println!(
                "  {:<18} rust {:<12} original {}",
                comparison.rust.case_name,
                short_result(&comparison.rust.result, 12),
                short_result(&original.result, 12),
            );
        }
        if mismatches.len() > 12 {
            println!("  ... {} more", mismatches.len() - 12);
        }
    }

    fn print_group_report(comparisons: &[BenchComparison], min_ratio: f64, color: bool) {
        print_section("Group Speedups", color);
        println!(
            "{:<10} {:>8} {:>12} {:<24} {:<24}",
            "group", "measured", "gmean", "best", "worst",
        );
        println!("{}", paint(color, ANSI_DIM, &"-".repeat(86)));
        for group in bench_group_order() {
            let group_comparisons: Vec<_> = comparisons
                .iter()
                .filter(|comparison| {
                    comparison.rust.group == group && comparison.original.is_some()
                })
                .collect();
            if group_comparisons.is_empty() {
                continue;
            }
            let gmean = geometric_mean(
                &group_comparisons
                    .iter()
                    .filter_map(|comparison| comparison.rust_vs_original_ratio())
                    .collect::<Vec<_>>(),
            );
            let mut best = group_comparisons[0];
            let mut worst = group_comparisons[0];
            for comparison in &group_comparisons {
                if comparison.rust_vs_original_ratio() > best.rust_vs_original_ratio() {
                    best = comparison;
                }
                if comparison.rust_vs_original_ratio() < worst.rust_vs_original_ratio() {
                    worst = comparison;
                }
            }
            println!(
                "{:<10} {:>8} {} {:<24} {:<24}",
                group.label(),
                group_comparisons.len(),
                paint(
                    color,
                    ratio_color(gmean, min_ratio),
                    &format!("{gmean:>12.3}x")
                ),
                format!(
                    "{} {:.2}x",
                    best.rust.case_name,
                    best.rust_vs_original_ratio().expect("measured original")
                ),
                format!(
                    "{} {:.2}x",
                    worst.rust.case_name,
                    worst.rust_vs_original_ratio().expect("measured original")
                ),
            );
        }
    }

    fn print_regression_report(comparisons: &[BenchComparison], min_ratio: f64, color: bool) {
        let threshold = min_ratio.max(1.0);
        let mut behind = Vec::new();
        for comparison in comparisons {
            if let Some(ratio) = comparison
                .rust_vs_original_ratio()
                .filter(|ratio| *ratio < threshold)
            {
                behind.push(("rust/orig", &comparison.rust, ratio));
            }
            if let Some(rust_cuda) = comparison.rust_cuda.as_ref() {
                if let Some(ratio) = comparison
                    .cuda_vs_original_ratio()
                    .filter(|ratio| *ratio < threshold)
                {
                    behind.push(("cuda/orig", rust_cuda, ratio));
                }
            }
        }
        behind.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(CmpOrdering::Equal));

        if behind.is_empty() {
            return;
        }

        print_section("Potential Regressions", color);
        println!(
            "{:<18} {:<10} {:>11} {:>13} note",
            "case", "relation", "ratio", "original faster",
        );
        println!("{}", paint(color, ANSI_DIM, &"-".repeat(87)));
        for (relation, summary, ratio_value) in behind.iter().take(8) {
            let ratio = paint(
                color,
                ratio_color(*ratio_value, threshold),
                &format!("{ratio_value:.3}x"),
            );
            println!(
                "{:<18} {:<10} {:>11} {:>12.1}% {}",
                summary.case_name,
                relation,
                ratio,
                (1.0 - *ratio_value) * 100.0,
                paint(color, ANSI_DIM, summary.note),
            );
        }
    }

    fn print_noise_report(comparisons: &[BenchComparison], color: bool) {
        let noisy: Vec<_> = comparisons
            .iter()
            .filter(|comparison| {
                comparison.rust.coefficient_variation > 0.05
                    || comparison
                        .rust_cuda
                        .as_ref()
                        .is_some_and(|rust_cuda| rust_cuda.coefficient_variation > 0.05)
                    || comparison
                        .original
                        .as_ref()
                        .is_some_and(|original| original.coefficient_variation > 0.05)
            })
            .collect();
        if noisy.is_empty() {
            return;
        }
        print_section("High Variance", color);
        for comparison in noisy {
            let original_cv = comparison.original.as_ref().map_or_else(
                || "n/a".to_owned(),
                |original| format!("{:>5.1}%", original.coefficient_variation * 100.0),
            );
            let cuda_cv = comparison.rust_cuda.as_ref().map_or_else(
                || "n/a".to_owned(),
                |rust_cuda| format!("{:>5.1}%", rust_cuda.coefficient_variation * 100.0),
            );
            println!(
                "  {:<18} rust cv {:>5.1}%   cuda cv {}   original cv {}   repeat or raise budget before trusting small deltas",
                comparison.rust.case_name,
                comparison.rust.coefficient_variation * 100.0,
                cuda_cv,
                original_cv,
            );
        }
    }

    fn print_original_tsv_compare(comparison: &BenchComparison, min_ratio: f64) {
        let Some(original) = &comparison.original else {
            if let Some(reason) = comparison.original_skip {
                println!(
                    "skip\toriginal\t{}\t{}\t{}\t{}\t\t\t{}\t\t\t\t\t\t\t\t\t\t{}",
                    comparison.rust.case_name,
                    comparison.rust.group.key(),
                    comparison.rust.shape.label(),
                    comparison.rust.budget,
                    comparison.rust.threads,
                    reason,
                );
            }
            return;
        };
        print_tsv_ratio(
            "rust-vs-original",
            &comparison.rust,
            original,
            comparison
                .rust_vs_original_ratio()
                .expect("original comparison has ratio"),
            min_ratio,
        );
    }

    fn print_section(title: &str, color: bool) {
        println!();
        println!("{}", paint(color, ANSI_DIM, title));
        println!("{}", paint(color, ANSI_DIM, &"-".repeat(title.len())));
    }

    fn print_tsv_header() {
        println!(
            "kind\timpl\tcase\tgroup\tshape\tbudget\tscanned\tscan_pct\tthreads\tsample\telapsed_ms\tseeds_per_sec\tns_per_seed\tmin_ms\tp50_ms\tp95_ms\tp99_ms\tmax_ms\tstdev_ms\tcv_pct\tresult"
        );
    }

    fn print_tsv_run(implementation: &str, case: &Case, run: &BenchRun) {
        println!(
            "run\t{}\t{}\t{}\t{}\t{}\t{}\t{:.6}\t{}\t{}\t{:.3}\t{:.0}\t{:.3}\t\t\t\t\t\t\t\t{}",
            implementation,
            case.name,
            case.group.key(),
            case.shape.label(),
            case.num_seeds,
            run.scanned,
            run.scanned as f64 / case.num_seeds as f64,
            case.threads,
            run.run,
            ms(run.elapsed),
            run.seeds_per_sec,
            run.ns_per_seed,
            run.result,
        );
    }

    fn print_tsv_summary(summary: &BenchSummary) {
        println!(
            "summary\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.6}\t{}\t{}\t{:.3}\t{:.0}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}",
            summary.implementation,
            summary.case_name,
            summary.group.key(),
            summary.shape.label(),
            summary.budget,
            summary.mean_scanned,
            summary.scanned_pct,
            summary.threads,
            summary.repeat,
            ms(summary.mean_elapsed),
            summary.seeds_per_sec,
            summary.ns_per_seed,
            ms(summary.min_elapsed),
            ms(summary.p50_elapsed),
            ms(summary.p95_elapsed),
            ms(summary.p99_elapsed),
            ms(summary.max_elapsed),
            ms(summary.stdev_elapsed),
            summary.coefficient_variation * 100.0,
            summary.result,
        );
    }

    fn print_tsv_compare(comparison: &BenchComparison, min_ratio: f64) {
        print_original_tsv_compare(comparison, min_ratio);
        if let Some(rust_cuda) = &comparison.rust_cuda {
            print_tsv_ratio(
                "rust-cuda-vs-rust-cpu",
                rust_cuda,
                &comparison.rust,
                comparison
                    .cuda_vs_cpu_ratio()
                    .expect("CUDA comparison has ratio"),
                1.0,
            );
            if let Some(original) = &comparison.original {
                print_tsv_ratio(
                    "rust-cuda-vs-original",
                    rust_cuda,
                    original,
                    comparison
                        .cuda_vs_original_ratio()
                        .expect("CUDA original comparison has ratio"),
                    min_ratio,
                );
            }
        }
    }

    fn print_tsv_ratio(
        relation: &str,
        lhs: &BenchSummary,
        rhs: &BenchSummary,
        ratio: f64,
        target_ratio: f64,
    ) {
        let status = if ratio >= target_ratio {
            "ok"
        } else {
            "below-target"
        };
        println!(
            "compare\t{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.6}\t{}\t{}\t{:.3}\t{:.0}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\tratio={:.3};target_ratio={:.3};lhs={};rhs={};lhs_sps={:.0};rhs_sps={:.0};lhs_ms={:.3};rhs_ms={:.3};lhs_result={};rhs_result={}",
            status,
            lhs.case_name,
            lhs.group.key(),
            lhs.shape.label(),
            lhs.budget,
            lhs.mean_scanned,
            lhs.scanned_pct,
            lhs.threads,
            lhs.repeat,
            ms(lhs.mean_elapsed),
            lhs.seeds_per_sec,
            lhs.ns_per_seed,
            ms(lhs.min_elapsed),
            ms(lhs.p50_elapsed),
            ms(lhs.p95_elapsed),
            ms(lhs.p99_elapsed),
            ms(lhs.max_elapsed),
            ms(lhs.stdev_elapsed),
            lhs.coefficient_variation * 100.0,
            ratio,
            target_ratio,
            relation.split("-vs-").next().unwrap_or(relation),
            relation.split("-vs-").nth(1).unwrap_or("unknown"),
            lhs.seeds_per_sec,
            rhs.seeds_per_sec,
            ms(lhs.mean_elapsed),
            ms(rhs.mean_elapsed),
            lhs.result,
            rhs.result,
        );
    }

    fn bench_group_order() -> [BenchGroup; 8] {
        [
            BenchGroup::Baseline,
            BenchGroup::Tags,
            BenchGroup::Vouchers,
            BenchGroup::Packs,
            BenchGroup::Jokers,
            BenchGroup::Souls,
            BenchGroup::Deck,
            BenchGroup::Ux,
        ]
    }

    fn geometric_mean(values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let mean_ln = values.iter().map(|value| value.ln()).sum::<f64>() / values.len() as f64;
        mean_ln.exp()
    }

    fn ratio_color(ratio: f64, min_ratio: f64) -> &'static str {
        if ratio < min_ratio {
            ANSI_RED
        } else if ratio < 1.0 {
            ANSI_YELLOW
        } else {
            ANSI_GREEN
        }
    }

    fn cv_color(coefficient_variation: f64) -> &'static str {
        if coefficient_variation > 0.05 {
            ANSI_RED
        } else if coefficient_variation > 0.02 {
            ANSI_YELLOW
        } else {
            ANSI_GREEN
        }
    }

    fn ms(duration: Duration) -> f64 {
        duration.as_secs_f64() * 1000.0
    }

    fn format_rate(value: f64) -> String {
        format!("{}/s", format_compact(value))
    }

    fn format_compact(value: f64) -> String {
        if value >= 1_000_000_000.0 {
            format!("{:.2}B", value / 1_000_000_000.0)
        } else if value >= 1_000_000.0 {
            format!("{:.2}M", value / 1_000_000.0)
        } else if value >= 1_000.0 {
            format!("{:.2}K", value / 1_000.0)
        } else {
            format!("{value:.0}")
        }
    }

    fn format_ns(value: f64) -> String {
        if value >= 1_000_000.0 {
            format!("{:.2}ms", value / 1_000_000.0)
        } else if value >= 1_000.0 {
            format!("{:.2}us", value / 1_000.0)
        } else {
            format!("{value:.1}ns")
        }
    }

    fn format_integer(value: i64) -> String {
        let negative = value < 0;
        let mut chars: Vec<_> = value.abs().to_string().chars().rev().collect();
        let mut out = String::new();
        for idx in 0..chars.len() {
            if idx > 0 && idx % 3 == 0 {
                out.push(',');
            }
            out.push(chars[idx]);
        }
        if negative {
            out.push('-');
        }
        chars.clear();
        out.chars().rev().collect()
    }

    fn short_result(value: &str, width: usize) -> String {
        if value.chars().count() <= width {
            return value.to_owned();
        }
        let mut out: String = value.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    }

    fn selected_bench_cases(
        selected_case: &str,
        budget: i64,
        threads: i32,
    ) -> Result<Vec<Case>, String> {
        bench::selected_bench_cases(selected_case).map(|cases| {
            cases
                .into_iter()
                .map(|case| case_from_bench_case(case, budget, threads))
                .collect()
        })
    }

    fn case_from_bench_case(case: BenchCase, budget: i64, threads: i32) -> Case {
        Case {
            name: case.name,
            group: case.group,
            shape: case.shape,
            note: case.note,
            seed_start: Some(case.seed_start),
            voucher: Some(case.voucher),
            pack: Some(case.pack),
            tag1: Some(case.tag1),
            tag2: Some(case.tag2),
            joker: Some(case.joker),
            joker_location: Some(case.joker_location),
            souls: case.souls,
            observatory: case.observatory,
            perkeo: case.perkeo,
            deck: Some(case.deck),
            erratic: case.erratic,
            no_faces: case.no_faces,
            min_face_cards: case.min_face_cards,
            suit_ratio: case.suit_ratio,
            num_seeds: budget,
            threads,
        }
    }
}
