#![allow(unsafe_code)]

#[cfg(not(windows))]
fn main() {
    eprintln!("immolate_dll_harness must be built for Windows and run under Windows or Wine");
    std::process::exit(2);
}

#[cfg(any(windows, test))]
const LEGACY_SEED_SPACE: i64 = 2_318_107_019_761;

#[cfg(any(windows, test))]
#[derive(Debug, Eq, PartialEq)]
enum LegacyProbe {
    EmptyResult,
    Hit { scanned: i64 },
}

#[cfg(any(windows, test))]
fn legacy_seed_id(seed: &str) -> Result<i64, String> {
    if seed.len() > 8 {
        return Err(format!("legacy seed is longer than eight bytes: {seed:?}"));
    }
    if seed.is_empty() {
        return Ok(0);
    }

    let mut shorter_lengths = 0_i64;
    let mut length_size = 35_i64;
    for _ in 1..seed.len() {
        shorter_lengths += length_size;
        length_size *= 35;
    }

    let mut within_length = 0_i64;
    for byte in seed.bytes() {
        let digit = match byte {
            b'1'..=b'9' => i64::from(byte - b'1'),
            b'A'..=b'Z' => i64::from(byte - b'A' + 9),
            _ => return Err(format!("legacy seed contains an invalid byte: {seed:?}")),
        };
        within_length = within_length * 35 + digit;
    }
    Ok(shorter_lengths + within_length + 1)
}

#[cfg(any(windows, test))]
fn legacy_seed_scan_count(start: &str, result: &str) -> Result<i64, String> {
    let start = legacy_seed_id(start)?;
    let result = legacy_seed_id(result)?;
    Ok((result - start).rem_euclid(LEGACY_SEED_SPACE) + 1)
}

#[cfg(any(windows, test))]
fn classify_legacy_probe(start: &str, result: Option<&str>) -> Result<LegacyProbe, String> {
    let Some(result) = result else {
        return Err("legacy DLL returned a null pointer".to_owned());
    };
    if result.is_empty() {
        return Ok(LegacyProbe::EmptyResult);
    }
    let scanned = legacy_seed_scan_count(start, result)?;
    Ok(LegacyProbe::Hit { scanned })
}

#[cfg(any(windows, test))]
fn is_strict_legacy_comparison(
    case_name: &str,
    rust_result: &str,
    rust_scanned: i64,
    legacy_result: &str,
    legacy_scanned: i64,
) -> bool {
    case_name == "baseline-hit"
        && !rust_result.is_empty()
        && rust_result != "<null>"
        && legacy_result != "<null>"
        && rust_result == legacy_result
        && rust_scanned == 1
        && legacy_scanned == 1
}

#[cfg(any(windows, test))]
fn legacy_empty_proves_mismatch(rust_result: &str) -> bool {
    !matches!(rust_result, "" | "<null>")
}

#[cfg(any(windows, test))]
fn requires_strict_legacy_fixture(selected_case: &str) -> bool {
    matches!(selected_case, "all" | "baseline" | "baseline-hit")
}

#[cfg(test)]
mod tests {
    use super::{
        LEGACY_SEED_SPACE, LegacyProbe, classify_legacy_probe, is_strict_legacy_comparison,
        legacy_empty_proves_mismatch, legacy_seed_id, legacy_seed_scan_count,
        requires_strict_legacy_fixture,
    };

    #[test]
    fn legacy_seed_ids_follow_length_major_lexicographic_order() {
        assert_eq!(legacy_seed_id(""), Ok(0));
        assert_eq!(legacy_seed_id("1"), Ok(1));
        assert_eq!(legacy_seed_id("A"), Ok(10));
        assert_eq!(legacy_seed_id("Z"), Ok(35));
        assert_eq!(legacy_seed_id("11"), Ok(36));
        assert_eq!(legacy_seed_id("1D"), Ok(48));
        assert_eq!(legacy_seed_id("IA"), Ok(640));
        assert_eq!(legacy_seed_id("ZZZZZZZZ"), Ok(LEGACY_SEED_SPACE - 1));
        assert!(legacy_seed_id("0").is_err());
        assert!(legacy_seed_id("111111111").is_err());
    }

    #[test]
    fn legacy_probe_classifies_hits_and_ambiguous_empty_results() {
        assert_eq!(legacy_seed_scan_count("A", "A"), Ok(1));
        assert_eq!(legacy_seed_scan_count("", "A"), Ok(11));
        assert_eq!(legacy_seed_scan_count("ZZZZZZZZ", "1"), Ok(3));
        assert_eq!(
            classify_legacy_probe("", Some("A")),
            Ok(LegacyProbe::Hit { scanned: 11 })
        );
        assert_eq!(
            classify_legacy_probe("", Some("")),
            Ok(LegacyProbe::EmptyResult)
        );
        assert!(classify_legacy_probe("", None).is_err());
    }

    #[test]
    fn strict_legacy_comparison_is_the_proven_nonempty_one_candidate_fixture() {
        assert!(is_strict_legacy_comparison("baseline-hit", "1", 1, "1", 1,));
        assert!(!is_strict_legacy_comparison("other", "1", 1, "1", 1));
        assert!(!is_strict_legacy_comparison(
            "baseline-hit",
            "A",
            11,
            "A",
            11,
        ));
        assert!(!is_strict_legacy_comparison("baseline-hit", "", 1, "", 1,));
        assert!(!is_strict_legacy_comparison(
            "baseline-hit",
            "<null>",
            1,
            "<null>",
            1,
        ));
        assert!(!is_strict_legacy_comparison("baseline-hit", "B", 1, "A", 1,));
    }

    #[test]
    fn ambiguous_legacy_empty_only_disproves_a_nonempty_current_seed() {
        assert!(!legacy_empty_proves_mismatch(""));
        assert!(!legacy_empty_proves_mismatch("<null>"));
        assert!(legacy_empty_proves_mismatch("1"));
    }

    #[test]
    fn full_and_baseline_selections_require_the_strict_fixture() {
        assert!(requires_strict_legacy_fixture("all"));
        assert!(requires_strict_legacy_fixture("baseline"));
        assert!(requires_strict_legacy_fixture("baseline-hit"));
        assert!(!requires_strict_legacy_fixture("ux"));
        assert!(!requires_strict_legacy_fixture("ux-soul-no-pack"));
    }
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

    use immolate::engine::config::{CompiledFilter, KernelShape};
    use immolate::filters::FilterConfig;
    use immolate::seed::{SEED_SPACE, Seed};

    use super::bench_cases::{self as bench, BenchCase, BenchGroup, BenchShape};
    use super::{
        LegacyProbe, classify_legacy_probe, is_strict_legacy_comparison,
        legacy_empty_proves_mismatch, legacy_seed_scan_count, requires_strict_legacy_fixture,
    };

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
        compiled_no_match: bool,
    }

    struct Dll {
        handle: HModule,
        entry: DllEntry,
        free_result: FreeResult,
    }

    enum DllEntry {
        Current(BrainstormSearch),
        Original(OriginalBrainstorm),
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    enum MeasurementKind {
        Current,
        Legacy,
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
            let search_ptr = unsafe { GetProcAddress(handle, search_name.as_ptr()) };
            let free_ptr = unsafe { GetProcAddress(handle, free_name.as_ptr()) };
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
            })
        }

        fn run(&self, case: &Case) -> Result<Option<String>, String> {
            match self.entry {
                DllEntry::Current(search) => self.run_current(case, search),
                DllEntry::Original(search) => self.run_original(case, search),
            }
        }

        fn measurement_kind(&self) -> MeasurementKind {
            match &self.entry {
                DllEntry::Current(_) => MeasurementKind::Current,
                DllEntry::Original(_) => MeasurementKind::Legacy,
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
                (self.free_result)(result.cast_mut());
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
                if let Err(err) = bench(&dll, settings) {
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
                if let Err(err) =
                    bench_compare(&rust, &original, settings, min_ratio, fail_on_mismatch)
                {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            },
            Err(err) => {
                eprintln!("{err}");
                eprintln!(
                    "usage:\n  immolate_dll_harness bench --dll PATH [--case all|GROUP|NAME] [--budget N] [--threads N] [--repeat N] [--warmup N] [--format pretty|tsv] [--color auto|always|never]\n  immolate_dll_harness bench-compare --rust PATH --original PATH [--case all|GROUP|NAME] [--budget N] [--threads N] [--repeat N] [--warmup N] [--min-ratio N] [--fail-on-mismatch true|false] [--format pretty|tsv] [--color auto|always|never]"
                );
                std::process::exit(2);
            },
        }
    }

    fn bench(dll_path: &str, settings: BenchSettings<'_>) -> Result<(), String> {
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
            print_run_header(
                "Brainstorm Supercharged DLL Benchmark",
                settings,
                cases.len(),
            );
        }

        let mut summaries = Vec::with_capacity(cases.len());
        for case in &cases {
            summaries.push(measure_bench_case(
                &dll,
                case,
                settings.repeat,
                settings.warmup,
                "dll",
                settings.output,
            )?);
        }
        if settings.output.format == OutputFormat::Pretty {
            print_single_bench_report(&summaries, settings.output);
        }
        Ok(())
    }

    fn bench_compare(
        rust_path: &str,
        original_path: &str,
        settings: BenchSettings<'_>,
        min_ratio: f64,
        fail_on_mismatch: bool,
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
                "Brainstorm Supercharged DLL Benchmark: Rust vs Original",
                settings,
                cases.len(),
            );
        }

        let mut failed = false;
        let mut comparisons = Vec::with_capacity(cases.len());
        for case in &cases {
            let rust_probe = rust.run(case)?;
            let rust_probe_scanned =
                measured_scanned_count(rust.measurement_kind(), case, rust_probe.as_deref())?;
            let rust_probe_result = display_result(rust_probe.as_deref()).to_owned();
            let rust_summary = measure_bench_case(
                &rust,
                case,
                settings.repeat,
                settings.warmup,
                "rust",
                settings.output,
            )?;
            if rust_summary.result != rust_probe_result
                || rust_summary.scanned != rust_probe_scanned
            {
                return Err(format!(
                    "Rust DLL changed result during {}: probe={rust_probe_result}/{rust_probe_scanned}, measured={}/{}",
                    case.name, rust_summary.result, rust_summary.scanned,
                ));
            }
            let (original_summary, original_skip) = if let Some(reason) = original_skip_reason(case)
            {
                (None, Some(reason))
            } else {
                let probe_result = original.run(case)?;
                match classify_legacy_probe(case.seed_start.unwrap_or(""), probe_result.as_deref())?
                {
                    LegacyProbe::EmptyResult => {
                        if fail_on_mismatch && legacy_empty_proves_mismatch(&rust_summary.result) {
                            failed = true;
                            eprintln!(
                                "benchmark parity mismatch in {}: rust={} original=<ambiguous-empty>",
                                case.name, rust_summary.result,
                            );
                        }
                        (
                            None,
                            Some(
                                "legacy empty result is ambiguous between an initial-seed hit and a fixed-cap miss",
                            ),
                        )
                    },
                    LegacyProbe::Hit { scanned } => {
                        let probe_result =
                            probe_result.expect("non-empty legacy probe has a result");
                        let summary = measure_bench_case(
                            &original,
                            case,
                            settings.repeat,
                            settings.warmup,
                            "original",
                            settings.output,
                        )?;
                        if summary.result != probe_result || summary.scanned != scanned {
                            return Err(format!(
                                "legacy DLL changed result during {}: probe={probe_result}/{scanned}, measured={}/{}",
                                case.name, summary.result, summary.scanned,
                            ));
                        }
                        (Some(summary), None)
                    },
                }
            };
            let comparison = BenchComparison {
                rust: rust_summary,
                original: original_summary,
                original_skip,
            };
            if min_ratio > 0.0
                && comparison
                    .strict_rust_vs_original_ratio()
                    .is_some_and(|ratio| ratio < min_ratio)
            {
                failed = true;
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
        if min_ratio > 0.0
            && requires_strict_legacy_fixture(settings.selected_case)
            && !comparisons
                .iter()
                .any(BenchComparison::is_strictly_comparable)
        {
            failed = true;
            eprintln!(
                "benchmark selection {} did not produce its required strict baseline comparison",
                settings.selected_case,
            );
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
        scanned: i64,
        scanned_pct: f64,
        seeds_per_sec: f64,
        ns_per_seed: f64,
        result: String,
    }

    struct BenchComparison {
        rust: BenchSummary,
        original: Option<BenchSummary>,
        original_skip: Option<&'static str>,
    }

    impl BenchComparison {
        fn rust_vs_original_ratio(&self) -> Option<f64> {
            self.original.as_ref().map(|original| {
                original.mean_elapsed.as_secs_f64() / self.rust.mean_elapsed.as_secs_f64()
            })
        }

        fn is_strictly_comparable(&self) -> bool {
            self.original.as_ref().is_some_and(|original| {
                is_strict_legacy_comparison(
                    self.rust.case_name,
                    &self.rust.result,
                    self.rust.scanned,
                    &original.result,
                    original.scanned,
                )
            })
        }

        fn strict_rust_vs_original_ratio(&self) -> Option<f64> {
            self.is_strictly_comparable().then(|| {
                self.rust_vs_original_ratio()
                    .expect("strict comparison has original")
            })
        }

        fn has_result_mismatch(&self) -> bool {
            self.original
                .as_ref()
                .is_some_and(|original| self.rust.result != original.result)
        }
    }

    fn measure_bench_case(
        dll: &Dll,
        case: &Case,
        repeat: usize,
        warmup: usize,
        implementation: &'static str,
        output: OutputOptions,
    ) -> Result<BenchSummary, String> {
        let measurement_kind = dll.measurement_kind();
        run_warmups(dll, case, warmup)?;
        let mut runs: Vec<BenchRun> = Vec::with_capacity(repeat);
        for run in 1..=repeat {
            let started = Instant::now();
            let result = dll.run(case);
            let elapsed = started.elapsed();
            let result = result?;
            if case.compiled_no_match && result.is_some() {
                return Err(format!(
                    "{} compiled to NoMatch but {implementation} returned a seed",
                    case.name,
                ));
            }
            let scanned = measured_scanned_count(measurement_kind, case, result.as_deref())?;
            let elapsed_secs = elapsed.as_secs_f64();
            let seeds_per_sec = if scanned > 0 && elapsed_secs > 0.0 {
                scanned as f64 / elapsed_secs
            } else {
                0.0
            };
            let ns_per_seed = if scanned > 0 {
                elapsed_secs * 1_000_000_000.0 / scanned as f64
            } else {
                0.0
            };
            let result = display_result(result.as_deref()).to_owned();
            if let Some(first) = runs.first() {
                if first.scanned != scanned || first.result != result {
                    return Err(format!(
                        "{} changed result during {implementation}: first={}/{}, run {run}={result}/{scanned}",
                        case.name, first.result, first.scanned,
                    ));
                }
            }
            let bench_run = BenchRun {
                run,
                elapsed,
                scanned,
                seeds_per_sec,
                ns_per_seed,
                result,
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
        let coefficient_variation = if mean_elapsed.is_zero() {
            0.0
        } else {
            stdev_elapsed.as_secs_f64() / mean_elapsed.as_secs_f64()
        };
        let scanned = runs[0].scanned;
        let scanned_f64 = scanned as f64;
        let seeds_per_sec = if scanned > 0 && !mean_elapsed.is_zero() {
            scanned_f64 / mean_elapsed.as_secs_f64()
        } else {
            0.0
        };
        let ns_per_seed = if scanned > 0 {
            mean_elapsed.as_secs_f64() * 1_000_000_000.0 / scanned_f64
        } else {
            0.0
        };
        let scanned_pct = scanned_f64 / case.num_seeds as f64;
        let result = runs[0].result.clone();
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
            scanned,
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

    fn display_result(result: Option<&str>) -> &str {
        result.unwrap_or("<null>")
    }

    fn measured_scanned_count(
        measurement_kind: MeasurementKind,
        case: &Case,
        result: Option<&str>,
    ) -> Result<i64, String> {
        if measurement_kind == MeasurementKind::Current {
            return Ok(scanned_count(case, result));
        }
        let Some(result) = result else {
            return Err(format!(
                "legacy DLL returned a null pointer during {}",
                case.name
            ));
        };
        if result.is_empty() {
            return Err(format!(
                "legacy DLL returned an ambiguous empty result during {}",
                case.name,
            ));
        }
        legacy_seed_scan_count(case.seed_start.unwrap_or(""), result)
    }

    fn scanned_count(case: &Case, result: Option<&str>) -> i64 {
        if case.compiled_no_match {
            return 0;
        }
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
        if case.compiled_no_match {
            return Some("current engine rejects this filter combination without scanning");
        }
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
        print_section("Rust vs Original Brainstorm", color);
        println!(
            "{}",
            paint(
                color,
                ANSI_DIM,
                "~ ratios are informational; BENCH_MIN_RATIO applies only to the proven baseline-hit fixture",
            )
        );
        println!(
            "{:<18} {:<9} {:<6} {:>7} {:>11} {:>11} {:>11} {:>17} {:>17} {:>11}",
            "case",
            "group",
            "shape",
            "scan",
            "rust/s",
            "original/s",
            "rust/orig",
            "mean ms R/O",
            "ns/seed R/O",
            "cv R/O",
        );
        println!("{}", paint(color, ANSI_DIM, &"-".repeat(133)));
        for comparison in comparisons {
            if let Some(original) = &comparison.original {
                let rust_ratio = comparison
                    .rust_vs_original_ratio()
                    .expect("original comparison has ratio");
                let strict = comparison.is_strictly_comparable();
                let ratio_text = if strict {
                    format!("{rust_ratio:.3}x")
                } else {
                    format!("~{rust_ratio:.3}x")
                };
                let ratio_text = format!("{ratio_text:>11}");
                let ratio = paint(
                    color,
                    if strict {
                        ratio_color(rust_ratio, min_ratio.max(1.0))
                    } else {
                        ANSI_DIM
                    },
                    &ratio_text,
                );
                let cv_pair = format!(
                    "{:.1}/{:.1}%",
                    comparison.rust.coefficient_variation * 100.0,
                    original.coefficient_variation * 100.0,
                );
                println!(
                    "{:<18} {:<9} {:<6} {:>7} {:>11} {:>11} {} {:>17} {:>17} {:>11}",
                    comparison.rust.case_name,
                    comparison.rust.group.label(),
                    comparison.rust.shape.label(),
                    format!("{:.1}%", comparison.rust.scanned_pct * 100.0),
                    format_rate(comparison.rust.seeds_per_sec),
                    format_rate(original.seeds_per_sec),
                    ratio,
                    format!(
                        "{:.3}/{:.3}",
                        ms(comparison.rust.mean_elapsed),
                        ms(original.mean_elapsed)
                    ),
                    format!(
                        "{}/{}",
                        format_ns(comparison.rust.ns_per_seed),
                        format_ns(original.ns_per_seed)
                    ),
                    cv_pair,
                );
            } else if let Some(reason) = comparison.original_skip {
                println!(
                    "{:<18} {:<9} {:<6} {:>7} {:>11} {:>11} {:>11} {:>17} {:>17} {:>11} {}",
                    comparison.rust.case_name,
                    comparison.rust.group.label(),
                    comparison.rust.shape.label(),
                    format!("{:.1}%", comparison.rust.scanned_pct * 100.0),
                    format_rate(comparison.rust.seeds_per_sec),
                    "skipped",
                    "n/a",
                    format!("{:.3}/n/a", ms(comparison.rust.mean_elapsed)),
                    format!("{}/n/a", format_ns(comparison.rust.ns_per_seed)),
                    format!("{:.1}/n/a%", comparison.rust.coefficient_variation * 100.0),
                    paint(color, ANSI_DIM, reason),
                );
            }
        }
        print_result_mismatch_report(comparisons, color);
        print_group_report(comparisons, min_ratio, color);
        print_regression_report(comparisons, min_ratio, color);
        print_noise_report(comparisons, color);
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
        print_section("Strict Group Speedups", color);
        println!(
            "{:<10} {:>8} {:>12} {:<24} {:<24}",
            "group", "measured", "gmean", "best", "worst",
        );
        println!("{}", paint(color, ANSI_DIM, &"-".repeat(86)));
        for group in bench_group_order() {
            let group_comparisons: Vec<_> = comparisons
                .iter()
                .filter(|comparison| {
                    comparison.rust.group == group && comparison.is_strictly_comparable()
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
        let mut behind: Vec<_> = comparisons
            .iter()
            .filter(|comparison| {
                comparison
                    .strict_rust_vs_original_ratio()
                    .is_some_and(|ratio| ratio < threshold)
            })
            .collect();
        behind.sort_by(|a, b| {
            a.rust_vs_original_ratio()
                .expect("measured original")
                .partial_cmp(&b.rust_vs_original_ratio().expect("measured original"))
                .unwrap_or(CmpOrdering::Equal)
        });

        if behind.is_empty() {
            return;
        }

        print_section("Potential Regressions", color);
        println!(
            "{:<18} {:>11} {:>13} note",
            "case", "rust/orig", "original faster",
        );
        println!("{}", paint(color, ANSI_DIM, &"-".repeat(76)));
        for comparison in behind.iter().take(8) {
            let rust_ratio = comparison
                .rust_vs_original_ratio()
                .expect("measured original");
            let ratio = paint(
                color,
                ratio_color(rust_ratio, threshold),
                &format!("{rust_ratio:.3}x"),
            );
            println!(
                "{:<18} {:>11} {:>12.1}% {}",
                comparison.rust.case_name,
                ratio,
                (1.0 - rust_ratio) * 100.0,
                paint(color, ANSI_DIM, comparison.rust.note),
            );
        }
    }

    fn print_noise_report(comparisons: &[BenchComparison], color: bool) {
        let noisy: Vec<_> = comparisons
            .iter()
            .filter(|comparison| {
                comparison.rust.coefficient_variation > 0.05
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
            println!(
                "  {:<18} rust cv {:>5.1}%   original cv {}   repeat or raise budget before trusting small deltas",
                comparison.rust.case_name,
                comparison.rust.coefficient_variation * 100.0,
                original_cv,
            );
        }
    }

    fn print_original_tsv_compare(comparison: &BenchComparison, min_ratio: f64) {
        let Some(original) = &comparison.original else {
            if let Some(reason) = comparison.original_skip {
                let fields = [
                    "skip".to_owned(),
                    "original".to_owned(),
                    comparison.rust.case_name.to_owned(),
                    comparison.rust.group.key().to_owned(),
                    comparison.rust.shape.label().to_owned(),
                    comparison.rust.budget.to_string(),
                    String::new(),
                    String::new(),
                    comparison.rust.threads.to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    reason.to_owned(),
                ];
                println!("{}", fields.join("\t"));
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
            comparison.is_strictly_comparable(),
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
            "summary\t{}\t{}\t{}\t{}\t{}\t{}\t{:.6}\t{}\t{}\t{:.3}\t{:.0}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}",
            summary.implementation,
            summary.case_name,
            summary.group.key(),
            summary.shape.label(),
            summary.budget,
            summary.scanned,
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
    }

    fn print_tsv_ratio(
        relation: &str,
        lhs: &BenchSummary,
        rhs: &BenchSummary,
        ratio: f64,
        target_ratio: f64,
        strict: bool,
    ) {
        let status = if !strict {
            "informational"
        } else if ratio >= target_ratio {
            "ok"
        } else {
            "below-target"
        };
        println!(
            "compare\t{}\t{}\t{}\t{}\t{}\t{}\t{:.6}\t{}\t{}\t{:.3}\t{:.0}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\tratio={:.3};target_ratio={:.3};strict={};lhs={};rhs={};lhs_sps={:.0};rhs_sps={:.0};lhs_ms={:.3};rhs_ms={:.3};lhs_result={};rhs_result={}",
            status,
            lhs.case_name,
            lhs.group.key(),
            lhs.shape.label(),
            lhs.budget,
            lhs.scanned,
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
            strict,
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
        bench::selected_bench_cases(selected_case)?
            .into_iter()
            .map(|case| case_from_bench_case(case, budget, threads))
            .collect()
    }

    fn case_from_bench_case(case: BenchCase, budget: i64, threads: i32) -> Result<Case, String> {
        let config = FilterConfig::from_raw(
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
        );
        let compiled_no_match = CompiledFilter::compile(&config).shape == KernelShape::NoMatch;
        if (case.shape == BenchShape::Static) != compiled_no_match {
            return Err(format!(
                "benchmark case {} has shape {}, but the filter compiler says {}",
                case.name,
                case.shape.label(),
                if compiled_no_match {
                    "static"
                } else {
                    "searchable"
                },
            ));
        }
        Ok(Case {
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
            compiled_no_match,
        })
    }
}
