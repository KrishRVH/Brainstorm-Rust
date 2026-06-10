#![allow(unsafe_code)]

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_uint, c_void};
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::engine::config::{CompiledFilter, KernelShape};
use crate::item::Item;
use crate::seed::{SEED_SPACE, Seed};

macro_rules! extern_cuda_fn {
    (fn($($arg:ty),* $(,)?) -> $ret:ty) => {
        unsafe extern "system" fn($($arg),*) -> $ret
    };
}

const CUDA_MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/brainstorm_cuda.cubin"));
const CUDA_SUCCESS: i32 = 0;
const NO_RESULT: u64 = u64::MAX;
const PROBE_LAUNCH_SEEDS: i64 = 16_384;
const PROBE_WINDOW_SEEDS: i64 = 65_536;
const LAUNCH_SEEDS: i64 = 2_000_000;
const BLOCK_SIZE: u32 = 256;
const GRID_SIZE: u32 = 4096;

type CUdevice = c_int;
type CUcontext = *mut c_void;
type CUmodule = *mut c_void;
type CUfunction = *mut c_void;
type CUstream = *mut c_void;
type CUdeviceptr = u64;
type CUresult = c_int;

#[repr(C)]
#[derive(Clone, Copy)]
struct CudaFilterParams {
    tag1: u32,
    tag2: u32,
    voucher: u32,
    pack: u32,
    flags: u32,
}

const FLAG_TAGS: u32 = 1 << 0;
const FLAG_VOUCHER: u32 = 1 << 1;
const FLAG_PACKS: u32 = 1 << 2;
const FLAG_OBSERVATORY: u32 = 1 << 3;
const FLAG_SOULS: u32 = 1 << 4;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CudaSearch {
    Fallback,
    Complete(Option<String>),
}

pub(crate) fn set_cuda_enabled(enabled: bool) {
    CUDA_ENABLED.store(enabled, Ordering::Relaxed);
}

pub(crate) fn reset_last_search_used() {
    LAST_SEARCH_USED_CUDA.set(false);
}

pub(crate) fn mark_last_search_used() {
    LAST_SEARCH_USED_CUDA.set(true);
}

pub(crate) fn last_search_used() -> bool {
    LAST_SEARCH_USED_CUDA.get()
}

pub(crate) fn search_cuda(start_seed: i64, cfg: &CompiledFilter, num_seeds: i64) -> CudaSearch {
    if !CUDA_ENABLED.load(Ordering::Relaxed) {
        return CudaSearch::Fallback;
    }
    if CUDA_MODULE.is_empty() {
        return CudaSearch::Fallback;
    }
    let Some(params) = compile_cuda_filter(cfg) else {
        return CudaSearch::Fallback;
    };

    let state = CUDA_STATE.get_or_init(|| Mutex::new(CudaState::default()));
    let Ok(mut guard) = state.lock() else {
        return CudaSearch::Fallback;
    };
    if guard.disabled {
        return CudaSearch::Fallback;
    }
    if guard.engine.is_none() {
        if let Ok(engine) = CudaEngine::new() {
            guard.engine = Some(engine);
        } else {
            guard.disabled = true;
            return CudaSearch::Fallback;
        }
    }
    let Some(engine) = guard.engine.as_ref() else {
        return CudaSearch::Fallback;
    };
    let result = engine.search(start_seed, &params, num_seeds);
    result.map_or_else(
        |_| {
            guard.engine = None;
            guard.disabled = true;
            CudaSearch::Fallback
        },
        CudaSearch::Complete,
    )
}

fn compile_cuda_filter(cfg: &CompiledFilter) -> Option<CudaFilterParams> {
    if cfg.raw.deck != Item::Red_Deck
        || cfg.raw.joker != Item::RETRY
        || cfg.raw.souls > 1
        || cfg.raw.perkeo
        || cfg.raw.erratic
        || (cfg.raw.souls > 0 && cfg.raw.observatory)
    {
        return None;
    }

    match cfg.shape {
        KernelShape::TagOnly
        | KernelShape::VoucherOnly
        | KernelShape::VoucherSecondPack
        | KernelShape::PackOnly
        | KernelShape::Observatory
        | KernelShape::TagObservatory
        | KernelShape::Souls
        | KernelShape::Composite => {},
        _ => return None,
    }

    let mut flags = 0;
    if cfg.raw.tag1 != Item::RETRY || cfg.raw.tag2 != Item::RETRY {
        flags |= FLAG_TAGS;
    }
    if cfg.raw.voucher != Item::RETRY || cfg.raw.observatory {
        flags |= FLAG_VOUCHER;
    }
    if (cfg.raw.pack != Item::RETRY && cfg.raw.pack != Item::Buffoon_Pack) || cfg.raw.observatory {
        flags |= FLAG_PACKS;
    }
    if cfg.raw.observatory {
        flags |= FLAG_OBSERVATORY;
    }
    if cfg.raw.souls > 0 {
        flags |= FLAG_SOULS;
    }

    if flags == 0 {
        return None;
    }

    Some(CudaFilterParams {
        tag1: item_id_or_zero(cfg.raw.tag1),
        tag2: item_id_or_zero(cfg.raw.tag2),
        voucher: item_id_or_zero(cfg.raw.voucher),
        pack: item_id_or_zero(cfg.raw.pack),
        flags,
    })
}

fn item_id_or_zero(item: Item) -> u32 {
    if item == Item::RETRY {
        0
    } else {
        u32::from(item as u16)
    }
}

static CUDA_ENABLED: AtomicBool = AtomicBool::new(false);
static CUDA_STATE: OnceLock<Mutex<CudaState>> = OnceLock::new();

thread_local! {
    static LAST_SEARCH_USED_CUDA: Cell<bool> = const { Cell::new(false) };
}

#[derive(Default)]
struct CudaState {
    engine: Option<CudaEngine>,
    disabled: bool,
}

struct CudaEngine {
    driver: CudaDriver,
    context: CUcontext,
    module: CUmodule,
    kernel: CUfunction,
    spectral_soul_kernel: CUfunction,
    d_best_offset: CUdeviceptr,
}

// SAFETY: every driver/context access is serialized by `CUDA_STATE`'s mutex.
unsafe impl Send for CudaEngine {}

impl CudaEngine {
    fn new() -> Result<Self, CudaError> {
        let driver = CudaDriver::load()?;
        let mut engine = Self {
            driver,
            context: ptr::null_mut(),
            module: ptr::null_mut(),
            kernel: ptr::null_mut(),
            spectral_soul_kernel: ptr::null_mut(),
            d_best_offset: 0,
        };
        unsafe {
            check((engine.driver.cu_init)(0))?;

            let mut device_count = 0;
            check((engine.driver.cu_device_get_count)(&raw mut device_count))?;
            if device_count <= 0 {
                return Err(CudaError::NoDevice);
            }

            let mut device = 0;
            check((engine.driver.cu_device_get)(&raw mut device, 0))?;

            check((engine.driver.cu_ctx_create)(
                &raw mut engine.context,
                0,
                device,
            ))?;
            // SAFETY: `cuCtxCreate` leaves the new context current on this thread.
            let _current = CurrentContext::assume_current(&engine.driver);

            check((engine.driver.cu_module_load_data_ex)(
                &raw mut engine.module,
                CUDA_MODULE.as_ptr().cast(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            ))?;

            let kernel_name = c"brainstorm_search_kernel";
            check((engine.driver.cu_module_get_function)(
                &raw mut engine.kernel,
                engine.module,
                kernel_name.as_ptr(),
            ))?;
            let spectral_soul_kernel_name = c"brainstorm_search_spectral_soul_kernel";
            check((engine.driver.cu_module_get_function)(
                &raw mut engine.spectral_soul_kernel,
                engine.module,
                spectral_soul_kernel_name.as_ptr(),
            ))?;

            check((engine.driver.cu_mem_alloc)(
                &raw mut engine.d_best_offset,
                mem::size_of::<u64>(),
            ))?;
        }
        Ok(engine)
    }

    fn search(
        &self,
        start_seed: i64,
        params: &CudaFilterParams,
        num_seeds: i64,
    ) -> Result<Option<String>, CudaError> {
        let mut remaining = num_seeds.clamp(0, SEED_SPACE);
        if remaining == 0 {
            return Ok(None);
        }
        let mut start_seed = start_seed.rem_euclid(SEED_SPACE);
        let _current = CurrentContext::push(&self.driver, self.context)?;

        while remaining > 0 {
            let until_wrap = SEED_SPACE - start_seed;
            let mut segment_remaining = remaining.min(until_wrap);
            let mut next_launch = first_launch_size(segment_remaining);
            while segment_remaining > 0 {
                let count = segment_remaining.min(next_launch);
                let found = self.search_launch(start_seed, count, params)?;
                if let Some(offset) = found {
                    let offset = i64::try_from(offset).map_err(|_| CudaError::InvalidOffset)?;
                    let id = (start_seed + offset).rem_euclid(SEED_SPACE);
                    return Ok(Some(Seed::from_id(id).to_string()));
                }
                remaining -= count;
                segment_remaining -= count;
                start_seed = (start_seed + count).rem_euclid(SEED_SPACE);
                next_launch = LAUNCH_SEEDS;
            }
        }
        Ok(None)
    }

    fn search_launch(
        &self,
        start_seed: i64,
        count: i64,
        params: &CudaFilterParams,
    ) -> Result<Option<u64>, CudaError> {
        let mut best_offset = NO_RESULT;
        unsafe {
            check((self.driver.cu_memcpy_htod)(
                self.d_best_offset,
                ptr::from_ref(&best_offset).cast(),
                mem::size_of::<u64>(),
            ))?;

            let mut start_seed_arg = start_seed;
            let mut count_arg = count;
            let mut params_arg = *params;
            let mut best_arg = self.d_best_offset;
            let mut args = [
                ptr::from_mut(&mut start_seed_arg).cast::<c_void>(),
                ptr::from_mut(&mut count_arg).cast::<c_void>(),
                ptr::from_mut(&mut params_arg).cast::<c_void>(),
                ptr::from_mut(&mut best_arg).cast::<c_void>(),
            ];
            let kernel = self.search_kernel(params);

            check((self.driver.cu_launch_kernel)(
                kernel,
                launch_grid_size(count),
                1,
                1,
                BLOCK_SIZE,
                1,
                1,
                0,
                ptr::null_mut(),
                args.as_mut_ptr(),
                ptr::null_mut(),
            ))?;
            check((self.driver.cu_memcpy_dtoh)(
                ptr::from_mut(&mut best_offset).cast(),
                self.d_best_offset,
                mem::size_of::<u64>(),
            ))?;
        }

        match best_offset {
            NO_RESULT => Ok(None),
            offset if offset < u64::try_from(count).map_err(|_| CudaError::InvalidOffset)? => {
                Ok(Some(offset))
            },
            _ => Err(CudaError::InvalidOffset),
        }
    }

    fn search_kernel(&self, params: &CudaFilterParams) -> CUfunction {
        if use_spectral_soul_kernel(params) {
            self.spectral_soul_kernel
        } else {
            self.kernel
        }
    }
}

struct CurrentContext<'a> {
    driver: &'a CudaDriver,
}

impl<'a> CurrentContext<'a> {
    unsafe fn assume_current(driver: &'a CudaDriver) -> Self {
        Self { driver }
    }

    fn push(driver: &'a CudaDriver, context: CUcontext) -> Result<Self, CudaError> {
        unsafe {
            check((driver.cu_ctx_push_current)(context))?;
        }
        Ok(Self { driver })
    }
}

impl Drop for CurrentContext<'_> {
    fn drop(&mut self) {
        let mut popped = ptr::null_mut();
        unsafe {
            let _ = (self.driver.cu_ctx_pop_current)(&raw mut popped);
        }
    }
}

fn use_spectral_soul_kernel(params: &CudaFilterParams) -> bool {
    let allowed_flags = FLAG_TAGS | FLAG_VOUCHER | FLAG_PACKS | FLAG_SOULS;
    params.flags & !allowed_flags == 0
        && params.flags & FLAG_PACKS != 0
        && params.flags & FLAG_SOULS != 0
        && matches!(params.pack, 305..=307)
}

fn launch_grid_size(count: i64) -> u32 {
    let count = u64::try_from(count).unwrap_or(0).max(1);
    let block_size = u64::from(BLOCK_SIZE);
    let blocks = count.div_ceil(block_size);
    u32::try_from(blocks.min(u64::from(GRID_SIZE))).unwrap_or(GRID_SIZE)
}

fn first_launch_size(segment_seeds: i64) -> i64 {
    // Avoid queuing a full throughput grid before an early result can stop it.
    if segment_seeds >= PROBE_WINDOW_SEEDS {
        PROBE_LAUNCH_SEEDS
    } else {
        LAUNCH_SEEDS
    }
}

impl Drop for CudaEngine {
    fn drop(&mut self) {
        if self.context.is_null() {
            return;
        }
        if let Ok(_current) = CurrentContext::push(&self.driver, self.context) {
            unsafe {
                if self.d_best_offset != 0 {
                    let _ = (self.driver.cu_mem_free)(self.d_best_offset);
                }
                if !self.module.is_null() {
                    let _ = (self.driver.cu_module_unload)(self.module);
                }
            }
        }
        unsafe {
            // Destroy the detached context after the guard restores the caller.
            let _ = (self.driver.cu_ctx_destroy)(self.context);
        }
    }
}

#[derive(Debug)]
enum CudaError {
    Load,
    Symbol,
    NoDevice,
    InvalidOffset,
    Driver,
}

fn check(result: CUresult) -> Result<(), CudaError> {
    if result == CUDA_SUCCESS {
        Ok(())
    } else {
        Err(CudaError::Driver)
    }
}

struct CudaDriver {
    _lib: DynamicLibrary,
    cu_init: extern_cuda_fn!(fn(c_uint) -> CUresult),
    cu_device_get: extern_cuda_fn!(fn(*mut CUdevice, c_int) -> CUresult),
    cu_device_get_count: extern_cuda_fn!(fn(*mut c_int) -> CUresult),
    cu_ctx_create: extern_cuda_fn!(fn(*mut CUcontext, c_uint, CUdevice) -> CUresult),
    cu_ctx_destroy: extern_cuda_fn!(fn(CUcontext) -> CUresult),
    #[cfg(test)]
    cu_ctx_get_current: extern_cuda_fn!(fn(*mut CUcontext) -> CUresult),
    cu_ctx_push_current: extern_cuda_fn!(fn(CUcontext) -> CUresult),
    cu_ctx_pop_current: extern_cuda_fn!(fn(*mut CUcontext) -> CUresult),
    cu_module_load_data_ex: extern_cuda_fn!(
        fn(*mut CUmodule, *const c_void, c_uint, *mut c_void, *mut c_void) -> CUresult
    ),
    cu_module_get_function:
        extern_cuda_fn!(fn(*mut CUfunction, CUmodule, *const c_char) -> CUresult),
    cu_module_unload: extern_cuda_fn!(fn(CUmodule) -> CUresult),
    cu_mem_alloc: extern_cuda_fn!(fn(*mut CUdeviceptr, usize) -> CUresult),
    cu_mem_free: extern_cuda_fn!(fn(CUdeviceptr) -> CUresult),
    cu_memcpy_htod: extern_cuda_fn!(fn(CUdeviceptr, *const c_void, usize) -> CUresult),
    cu_memcpy_dtoh: extern_cuda_fn!(fn(*mut c_void, CUdeviceptr, usize) -> CUresult),
    cu_launch_kernel: extern_cuda_fn!(
        fn(
            CUfunction,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            CUstream,
            *mut *mut c_void,
            *mut *mut c_void,
        ) -> CUresult
    ),
}

impl CudaDriver {
    fn load() -> Result<Self, CudaError> {
        let lib = DynamicLibrary::open_cuda()?;
        unsafe {
            Ok(Self {
                cu_init: lib.symbol(b"cuInit\0")?,
                cu_device_get: lib.symbol(b"cuDeviceGet\0")?,
                cu_device_get_count: lib.symbol(b"cuDeviceGetCount\0")?,
                cu_ctx_create: lib.symbol_any(&[b"cuCtxCreate_v2\0", b"cuCtxCreate\0"])?,
                cu_ctx_destroy: lib.symbol_any(&[b"cuCtxDestroy_v2\0", b"cuCtxDestroy\0"])?,
                #[cfg(test)]
                cu_ctx_get_current: lib.symbol(b"cuCtxGetCurrent\0")?,
                cu_ctx_push_current: lib
                    .symbol_any(&[b"cuCtxPushCurrent_v2\0", b"cuCtxPushCurrent\0"])?,
                cu_ctx_pop_current: lib
                    .symbol_any(&[b"cuCtxPopCurrent_v2\0", b"cuCtxPopCurrent\0"])?,
                cu_module_load_data_ex: lib.symbol(b"cuModuleLoadDataEx\0")?,
                cu_module_get_function: lib.symbol(b"cuModuleGetFunction\0")?,
                cu_module_unload: lib.symbol(b"cuModuleUnload\0")?,
                cu_mem_alloc: lib.symbol_any(&[b"cuMemAlloc_v2\0", b"cuMemAlloc\0"])?,
                cu_mem_free: lib.symbol_any(&[b"cuMemFree_v2\0", b"cuMemFree\0"])?,
                cu_memcpy_htod: lib.symbol_any(&[b"cuMemcpyHtoD_v2\0", b"cuMemcpyHtoD\0"])?,
                cu_memcpy_dtoh: lib.symbol_any(&[b"cuMemcpyDtoH_v2\0", b"cuMemcpyDtoH\0"])?,
                cu_launch_kernel: lib.symbol(b"cuLaunchKernel\0")?,
                _lib: lib,
            })
        }
    }
}

struct DynamicLibrary {
    handle: *mut c_void,
}

unsafe impl Send for DynamicLibrary {}

impl DynamicLibrary {
    fn open_cuda() -> Result<Self, CudaError> {
        for name in cuda_library_names() {
            if let Ok(lib) = Self::open(name) {
                return Ok(lib);
            }
        }
        Err(CudaError::Load)
    }

    unsafe fn symbol<T: Copy>(&self, name: &[u8]) -> Result<T, CudaError> {
        let ptr = unsafe { self.raw_symbol(name) };
        if ptr.is_null() {
            return Err(CudaError::Symbol);
        }
        Ok(unsafe { mem::transmute_copy(&ptr) })
    }

    unsafe fn symbol_any<T: Copy>(&self, names: &[&[u8]]) -> Result<T, CudaError> {
        for name in names {
            if let Ok(symbol) = unsafe { self.symbol(name) } {
                return Ok(symbol);
            }
        }
        Err(CudaError::Symbol)
    }
}

impl Drop for DynamicLibrary {
    fn drop(&mut self) {
        unsafe {
            close_library(self.handle);
        }
    }
}

#[cfg(windows)]
fn cuda_library_names() -> &'static [&'static [u8]] {
    &[b"nvcuda.dll\0"]
}

#[cfg(not(windows))]
fn cuda_library_names() -> &'static [&'static [u8]] {
    &[
        b"libcuda.so.1\0",
        b"libcuda.so\0",
        b"/usr/lib/wsl/lib/libcuda.so\0",
    ]
}

#[cfg(windows)]
impl DynamicLibrary {
    fn open(name: &[u8]) -> Result<Self, CudaError> {
        unsafe {
            let handle = LoadLibraryA(name.as_ptr().cast());
            if handle.is_null() {
                Err(CudaError::Load)
            } else {
                Ok(Self {
                    handle: handle.cast(),
                })
            }
        }
    }

    unsafe fn raw_symbol(&self, name: &[u8]) -> *mut c_void {
        unsafe { GetProcAddress(self.handle.cast(), name.as_ptr().cast()).cast() }
    }
}

#[cfg(not(windows))]
impl DynamicLibrary {
    fn open(name: &[u8]) -> Result<Self, CudaError> {
        unsafe {
            let handle = dlopen(name.as_ptr().cast(), 0x0001);
            if handle.is_null() {
                Err(CudaError::Load)
            } else {
                Ok(Self { handle })
            }
        }
    }

    unsafe fn raw_symbol(&self, name: &[u8]) -> *mut c_void {
        unsafe { dlsym(self.handle, name.as_ptr().cast()) }
    }
}

#[cfg(windows)]
unsafe fn close_library(handle: *mut c_void) {
    if !handle.is_null() {
        let _ = unsafe { FreeLibrary(handle.cast()) };
    }
}

#[cfg(not(windows))]
unsafe fn close_library(handle: *mut c_void) {
    if !handle.is_null() {
        let _ = unsafe { dlclose(handle) };
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryA(name: *const c_char) -> *mut c_void;
    fn GetProcAddress(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn FreeLibrary(handle: *mut c_void) -> c_int;
}

#[cfg(not(windows))]
#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::engine::kernels::apply_compiled_filter;
    use crate::engine::seed::SearchState;
    use crate::filters::FilterConfig;

    #[test]
    fn cuda_filter_compilation_matches_supported_surface() {
        assert_cuda_supported(raw_cfg(
            "",
            "",
            "tag_charm",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        ));
        assert_cuda_supported(raw_cfg(
            "v_telescope",
            "",
            "",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        ));
        assert_cuda_supported(raw_cfg(
            "",
            "p_spectral_mega_1",
            "",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        ));
        assert_cuda_supported(raw_cfg(
            "", "", "", "", "", 0.0, true, false, "b_red", false,
        ));
        assert_cuda_supported(raw_cfg(
            "",
            "",
            "tag_charm",
            "",
            "",
            0.0,
            true,
            false,
            "b_red",
            false,
        ));
        assert_cuda_supported(raw_cfg(
            "v_telescope",
            "p_spectral_mega_1",
            "tag_charm",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        ));

        let voucher_pack = raw_cfg(
            "v_telescope",
            "p_spectral_mega_1",
            "",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        );
        assert_eq!(
            CompiledFilter::compile(&voucher_pack).shape,
            KernelShape::VoucherSecondPack,
        );
        assert_cuda_supported(voucher_pack);
        assert_cuda_supported(raw_cfg(
            "v_telescope",
            "p_spectral_mega_1",
            "tag_charm",
            "",
            "",
            1.0,
            false,
            false,
            "b_red",
            false,
        ));

        assert_cuda_unsupported(FilterConfig::default());
        let no_match = raw_cfg(
            "",
            "",
            "tag_buffoon",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        );
        assert_eq!(
            CompiledFilter::compile(&no_match).shape,
            KernelShape::NoMatch
        );
        assert_cuda_unsupported(no_match);
        assert_cuda_unsupported(raw_cfg(
            "",
            "",
            "",
            "",
            "Blueprint",
            0.0,
            false,
            false,
            "b_red",
            false,
        ));
        assert_cuda_unsupported(raw_cfg(
            "",
            "p_arcana_mega_1",
            "",
            "",
            "",
            2.0,
            false,
            false,
            "b_red",
            false,
        ));
        assert_cuda_unsupported(raw_cfg(
            "", "", "", "", "", 1.0, true, false, "b_red", false,
        ));
        assert_cuda_unsupported(raw_cfg(
            "", "", "", "", "", 0.0, false, true, "b_red", false,
        ));
        assert_cuda_unsupported(raw_cfg(
            "",
            "",
            "",
            "",
            "",
            0.0,
            false,
            false,
            "b_erratic",
            true,
        ));
        assert_cuda_unsupported(raw_cfg(
            "v_telescope",
            "",
            "",
            "",
            "",
            0.0,
            false,
            false,
            "b_magic",
            false,
        ));
    }

    #[test]
    fn cuda_elides_always_true_first_buffoon_pack_constraint() {
        let tag_and_buffoon = raw_cfg(
            "",
            "p_buffoon_normal_1",
            "tag_charm",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        );
        let params = compile_cuda_filter(&CompiledFilter::compile(&tag_and_buffoon))
            .expect("tag plus Buffoon pack should still use CUDA for the tag check");
        assert_eq!(params.flags & FLAG_TAGS, FLAG_TAGS);
        assert_eq!(params.flags & FLAG_PACKS, 0);

        assert_cuda_unsupported(raw_cfg(
            "",
            "p_buffoon_normal_1",
            "",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        ));
    }

    #[test]
    fn cuda_launch_grid_scales_with_count_and_caps_at_max_grid() {
        assert_eq!(launch_grid_size(1), 1);
        assert_eq!(launch_grid_size(i64::from(BLOCK_SIZE)), 1);
        assert_eq!(launch_grid_size(i64::from(BLOCK_SIZE) + 1), 2);
        assert_eq!(
            launch_grid_size(i64::from(BLOCK_SIZE) * i64::from(GRID_SIZE) * 2),
            GRID_SIZE,
        );
    }

    #[test]
    fn large_segments_start_with_a_bounded_probe_launch() {
        assert_eq!(first_launch_size(PROBE_WINDOW_SEEDS - 1), LAUNCH_SEEDS);
        assert_eq!(first_launch_size(PROBE_WINDOW_SEEDS), PROBE_LAUNCH_SEEDS);
    }

    #[test]
    fn cuda_item_ids_match_the_kernel_contract() {
        assert_eq!(mem::size_of::<Item>(), mem::size_of::<u16>());
        assert_eq!(Item::Overstock as u16, 162);
        assert_eq!(Item::Telescope as u16, 172);
        assert_eq!(Item::Palette as u16, 193);
        assert_eq!(Item::The_Soul as u16, 264);
        assert_eq!(Item::Black_Hole as u16, 265);
        assert_eq!(Item::Arcana_Pack as u16, 293);
        assert_eq!(Item::Mega_Arcana_Pack as u16, 295);
        assert_eq!(Item::Mega_Celestial_Pack as u16, 298);
        assert_eq!(Item::Buffoon_Pack as u16, 302);
        assert_eq!(Item::Spectral_Pack as u16, 305);
        assert_eq!(Item::Mega_Spectral_Pack as u16, 307);
        assert_eq!(Item::Uncommon_Tag as u16, 310);
        assert_eq!(Item::Charm_Tag as u16, 320);
        assert_eq!(Item::Economy_Tag as u16, 333);
        assert_eq!(Item::Red_Deck as u16, 445);
    }

    #[test]
    fn cuda_usage_marker_is_thread_local_and_resettable() {
        reset_last_search_used();
        assert!(!last_search_used());

        mark_last_search_used();
        assert!(last_search_used());
        assert!(matches!(
            std::thread::spawn(last_search_used).join(),
            Ok(false)
        ));
        assert!(last_search_used());

        reset_last_search_used();
        assert!(!last_search_used());
    }

    #[test]
    fn core_search_resets_cuda_usage_marker_before_early_returns() {
        mark_last_search_used();
        let _ = crate::brainstorm_search_core("", &FilterConfig::default(), 1, 1);
        assert!(!last_search_used());
    }

    #[test]
    #[ignore = "requires a compiled CUDA module, an NVIDIA driver, and a CUDA device"]
    fn cuda_restores_the_callers_current_context() {
        let driver = CudaDriver::load().expect("CUDA driver should load");
        unsafe {
            check((driver.cu_init)(0)).expect("CUDA should initialize");
        }
        let original = current_context(&driver);

        let mut device = 0;
        let mut caller = ptr::null_mut();
        unsafe {
            check((driver.cu_device_get)(&raw mut device, 0)).expect("device zero should exist");
            check((driver.cu_ctx_create)(&raw mut caller, 0, device))
                .expect("caller context should be created");
        }
        assert_eq!(current_context(&driver), caller);

        let params = compile_cuda_filter(&CompiledFilter::compile(&raw_cfg(
            "",
            "",
            "tag_charm",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        )))
        .expect("tag filter should compile for CUDA");
        let engine = CudaEngine::new().expect("CUDA engine should initialize");
        assert_eq!(current_context(&driver), caller);

        engine
            .search(0, &params, 1_000)
            .expect("CUDA search should complete");
        assert_eq!(current_context(&driver), caller);

        assert!(fail_while_engine_context_is_current(&engine).is_err());
        assert_eq!(current_context(&driver), caller);

        drop(engine);
        assert_eq!(current_context(&driver), caller);

        let mut popped = ptr::null_mut();
        unsafe {
            check((driver.cu_ctx_pop_current)(&raw mut popped)).expect("caller context should pop");
        }
        assert_eq!(popped, caller);
        assert_eq!(current_context(&driver), original);
        unsafe {
            check((driver.cu_ctx_destroy)(caller)).expect("caller context should be destroyed");
        }
    }

    #[test]
    #[ignore = "requires a compiled CUDA module, an NVIDIA driver, and a CUDA device"]
    #[allow(clippy::panic)]
    fn cuda_matches_cpu_results_and_scanned_counts() {
        struct DisableCuda;
        impl Drop for DisableCuda {
            fn drop(&mut self) {
                set_cuda_enabled(false);
            }
        }

        let cases = [
            (
                "single tag",
                "",
                50_000,
                true,
                raw_cfg(
                    "",
                    "",
                    "tag_charm",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "duplicate tag",
                "",
                50_000,
                true,
                raw_cfg(
                    "", "", "tag_rare", "tag_rare", "", 0.0, false, false, "b_red", false,
                ),
            ),
            (
                "distinct tags",
                "",
                50_000,
                true,
                raw_cfg(
                    "",
                    "",
                    "tag_charm",
                    "tag_d_six",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "voucher",
                "",
                50_000,
                true,
                raw_cfg(
                    "v_telescope",
                    "",
                    "",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "voucher second pack",
                "",
                50_000,
                true,
                raw_cfg(
                    "v_telescope",
                    "p_spectral_mega_1",
                    "",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "Arcana pack",
                "",
                50_000,
                true,
                raw_cfg(
                    "",
                    "p_arcana_normal_1",
                    "",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "Celestial pack",
                "",
                50_000,
                true,
                raw_cfg(
                    "",
                    "p_celestial_mega_1",
                    "",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "Standard pack",
                "",
                50_000,
                true,
                raw_cfg(
                    "",
                    "p_standard_jumbo_1",
                    "",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "Buffoon second pack",
                "",
                50_000,
                true,
                raw_cfg(
                    "",
                    "p_buffoon_mega_1",
                    "",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "Spectral pack",
                "",
                50_000,
                true,
                raw_cfg(
                    "",
                    "p_spectral_jumbo_1",
                    "",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "Observatory",
                "",
                50_000,
                true,
                raw_cfg("", "", "", "", "", 0.0, true, false, "b_red", false),
            ),
            (
                "tag Observatory",
                "",
                50_000,
                true,
                raw_cfg(
                    "",
                    "",
                    "tag_charm",
                    "",
                    "",
                    0.0,
                    true,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "Arcana Soul",
                "",
                100_000,
                true,
                raw_cfg(
                    "",
                    "p_arcana_mega_1",
                    "",
                    "",
                    "",
                    1.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "Soul without selected pack",
                "",
                100_000,
                true,
                raw_cfg("", "", "", "", "", 1.0, false, false, "b_red", false),
            ),
            (
                "forced Buffoon elision",
                "",
                50_000,
                true,
                raw_cfg(
                    "",
                    "p_buffoon_normal_1",
                    "tag_charm",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "composite miss",
                "",
                100_000,
                false,
                raw_cfg(
                    "v_telescope",
                    "p_spectral_mega_1",
                    "tag_charm",
                    "tag_d_six",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "spectral soul composite",
                "OQMKIM11",
                100_000,
                false,
                raw_cfg(
                    "v_telescope",
                    "p_spectral_mega_1",
                    "tag_charm",
                    "",
                    "",
                    1.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
            (
                "seed-space wrap",
                "ZZZZZZZZ",
                100_000,
                true,
                raw_cfg(
                    "",
                    "p_spectral_mega_1",
                    "tag_charm",
                    "",
                    "",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                ),
            ),
        ];

        set_cuda_enabled(true);
        let _disable = DisableCuda;
        for (name, start, budget, should_match, cfg) in cases {
            let start_id = Seed::from(start).id();
            let compiled = CompiledFilter::compile(&cfg);
            let expected = search_cpu(start_id, budget, &compiled);
            assert_eq!(expected.is_some(), should_match, "fixture drift for {name}");
            let CudaSearch::Complete(actual) = search_cuda(start_id, &compiled, budget) else {
                panic!("CUDA unavailable for {name}");
            };
            assert_eq!(actual, expected, "result mismatch for {name}");
            assert_eq!(
                scanned_count(start_id, actual.as_deref(), budget),
                scanned_count(start_id, expected.as_deref(), budget),
                "scanned-count mismatch for {name}",
            );
        }

        let prefix_hit = raw_cfg(
            "",
            "",
            "tag_charm",
            "",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        );
        let _ = crate::brainstorm_search_core("", &prefix_hit, 100_000, 1);
        assert!(
            !last_search_used(),
            "a serial-prefix hit must report CPU use"
        );

        let composite_miss = raw_cfg(
            "v_telescope",
            "p_spectral_mega_1",
            "tag_charm",
            "tag_d_six",
            "",
            0.0,
            false,
            false,
            "b_red",
            false,
        );
        let expected = search_cpu(0, 100_000, &CompiledFilter::compile(&composite_miss));
        let actual = crate::brainstorm_search_core("", &composite_miss, 100_000, 1);
        assert_eq!(actual, expected);
        assert!(
            last_search_used(),
            "a completed kernel must report CUDA use"
        );

        reset_last_search_used();
        let unsupported = CompiledFilter::compile(&raw_cfg(
            "",
            "",
            "",
            "",
            "Blueprint",
            0.0,
            false,
            false,
            "b_red",
            false,
        ));
        assert_eq!(search_cuda(0, &unsupported, 100_000), CudaSearch::Fallback);
        assert!(
            !last_search_used(),
            "an unsupported filter must report CPU use"
        );

        let retained_context = cached_context().expect("CUDA engine should be cached");
        set_cuda_enabled(false);
        assert_eq!(cached_context(), Some(retained_context));
        let actual = crate::brainstorm_search_core("", &composite_miss, 100_000, 1);
        assert_eq!(actual, expected);
        assert!(
            !last_search_used(),
            "a disabled CUDA arm must report CPU use"
        );

        set_cuda_enabled(true);
        let actual = crate::brainstorm_search_core("", &composite_miss, 100_000, 1);
        assert_eq!(actual, expected);
        assert!(last_search_used());
        assert_eq!(cached_context(), Some(retained_context));
    }

    #[test]
    fn cuda_uses_specialized_kernel_only_for_selected_spectral_soul_filters() {
        let spectral = compile_cuda_filter(&CompiledFilter::compile(&raw_cfg(
            "v_telescope",
            "p_spectral_mega_1",
            "tag_charm",
            "tag_charm",
            "",
            1.0,
            false,
            false,
            "b_red",
            false,
        )))
        .expect("selected Spectral Soul filter should compile for CUDA");
        assert!(use_spectral_soul_kernel(&spectral));

        let arcana = compile_cuda_filter(&CompiledFilter::compile(&raw_cfg(
            "",
            "p_arcana_mega_1",
            "tag_charm",
            "",
            "",
            1.0,
            false,
            false,
            "b_red",
            false,
        )))
        .expect("selected Arcana Soul filter should compile for CUDA");
        assert!(!use_spectral_soul_kernel(&arcana));

        let observatory = compile_cuda_filter(&CompiledFilter::compile(&raw_cfg(
            "",
            "",
            "tag_charm",
            "",
            "",
            0.0,
            true,
            false,
            "b_red",
            false,
        )))
        .expect("tag Observatory filter should compile for CUDA");
        assert!(!use_spectral_soul_kernel(&observatory));
    }

    fn assert_cuda_supported(cfg: FilterConfig) {
        assert!(
            compile_cuda_filter(&CompiledFilter::compile(&cfg)).is_some(),
            "{cfg:?} should compile for CUDA",
        );
    }

    fn search_cpu(start_seed: i64, count: i64, cfg: &CompiledFilter) -> Option<String> {
        let mut state = SearchState::from_id(start_seed);
        for _ in 0..count {
            if apply_compiled_filter(&mut state, cfg) {
                return Some(state.seed.to_string());
            }
            state.next();
        }
        None
    }

    fn current_context(driver: &CudaDriver) -> CUcontext {
        let mut context = ptr::null_mut();
        unsafe {
            check((driver.cu_ctx_get_current)(&raw mut context))
                .expect("current CUDA context should be readable");
        }
        context
    }

    fn fail_while_engine_context_is_current(engine: &CudaEngine) -> Result<(), CudaError> {
        let _current = CurrentContext::push(&engine.driver, engine.context)?;
        Err(CudaError::Driver)
    }

    fn cached_context() -> Option<usize> {
        let state = CUDA_STATE.get()?;
        let guard = state.lock().ok()?;
        guard.engine.as_ref().map(|engine| engine.context as usize)
    }

    fn scanned_count(start_seed: i64, result: Option<&str>, budget: i64) -> i64 {
        result.map_or(budget, |seed| {
            (Seed::from(seed).id() - start_seed).rem_euclid(SEED_SPACE) + 1
        })
    }

    fn assert_cuda_unsupported(cfg: FilterConfig) {
        assert!(
            compile_cuda_filter(&CompiledFilter::compile(&cfg)).is_none(),
            "{cfg:?} should use the Rust CPU path",
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn raw_cfg(
        voucher: &str,
        pack: &str,
        tag1: &str,
        tag2: &str,
        joker: &str,
        souls: f64,
        observatory: bool,
        perkeo: bool,
        deck: &str,
        erratic: bool,
    ) -> FilterConfig {
        FilterConfig::from_raw(
            voucher,
            pack,
            tag1,
            tag2,
            joker,
            "any",
            souls,
            observatory,
            perkeo,
            deck,
            erratic,
            false,
            0,
            0.0,
        )
    }
}
