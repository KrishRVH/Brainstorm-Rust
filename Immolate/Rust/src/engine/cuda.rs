#![allow(unsafe_code)]

use std::env;
use std::ffi::{CString, c_char, c_int, c_uint, c_void};
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

const PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/brainstorm_cuda.ptx"));
const CUDA_SUCCESS: i32 = 0;
const NO_RESULT: u64 = u64::MAX;
const DEFAULT_LAUNCH_SEEDS: i64 = 2_000_000;
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
#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CudaSearch {
    Unsupported,
    Unavailable,
    Complete(Option<String>),
}

pub fn set_cuda_enabled(enabled: bool) {
    CUDA_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn search_cuda(seed_start: &str, cfg: &CompiledFilter, num_seeds: i64) -> CudaSearch {
    if !CUDA_ENABLED.load(Ordering::Relaxed) {
        return CudaSearch::Unsupported;
    }
    if PTX.trim().is_empty() {
        return CudaSearch::Unavailable;
    }
    let Some(params) = compile_cuda_filter(cfg) else {
        return CudaSearch::Unsupported;
    };

    let state = CUDA_STATE.get_or_init(|| Mutex::new(CudaState::default()));
    let Ok(mut guard) = state.lock() else {
        return CudaSearch::Unavailable;
    };
    if guard.disabled {
        return CudaSearch::Unavailable;
    }
    if guard.engine.is_none() {
        if let Ok(engine) = CudaEngine::new() {
            guard.engine = Some(engine);
        } else {
            guard.disabled = true;
            return CudaSearch::Unavailable;
        }
    }
    let Some(engine) = guard.engine.as_mut() else {
        return CudaSearch::Unavailable;
    };
    if let Ok(result) = engine.search(seed_start, &params, num_seeds) {
        CudaSearch::Complete(result)
    } else {
        guard.engine = None;
        guard.disabled = true;
        CudaSearch::Unavailable
    }
}

#[allow(dead_code)]
pub fn debug_seed_cuda(seed_id: i64) -> Option<[u64; 8]> {
    let state = CUDA_STATE.get_or_init(|| Mutex::new(CudaState::default()));
    let mut guard = state.lock().ok()?;
    if guard.engine.is_none() {
        guard.engine = CudaEngine::new().ok();
    }
    guard.engine.as_mut()?.debug_seed(seed_id).ok()
}

fn compile_cuda_filter(cfg: &CompiledFilter) -> Option<CudaFilterParams> {
    if cfg.raw.deck != Item::Red_Deck
        || cfg.raw.joker != Item::RETRY
        || cfg.raw.souls > 0
        || cfg.raw.perkeo
        || cfg.raw.erratic
    {
        return None;
    }

    match cfg.shape {
        KernelShape::TagOnly
        | KernelShape::VoucherOnly
        | KernelShape::PackOnly
        | KernelShape::Observatory
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
    if cfg.raw.pack != Item::RETRY || cfg.raw.observatory {
        flags |= FLAG_PACKS;
    }
    if cfg.raw.observatory {
        flags |= FLAG_OBSERVATORY;
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
    if item == Item::RETRY { 0 } else { item as u32 }
}

fn cuda_launch_seeds() -> i64 {
    env::var("BRAINSTORM_CUDA_LAUNCH_SEEDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_LAUNCH_SEEDS)
        .clamp(1, SEED_SPACE)
}

static CUDA_ENABLED: AtomicBool = AtomicBool::new(true);
static CUDA_STATE: OnceLock<Mutex<CudaState>> = OnceLock::new();

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
    debug_kernel: CUfunction,
    d_params: CUdeviceptr,
    d_best_offset: CUdeviceptr,
    d_debug: CUdeviceptr,
}

unsafe impl Send for CudaEngine {}

impl CudaEngine {
    fn new() -> Result<Self, CudaError> {
        let driver = CudaDriver::load()?;
        unsafe {
            check((driver.cu_init)(0))?;

            let mut device_count = 0;
            check((driver.cu_device_get_count)(&raw mut device_count))?;
            if device_count <= 0 {
                return Err(CudaError::NoDevice);
            }

            let mut device = 0;
            check((driver.cu_device_get)(&raw mut device, 0))?;

            let mut context = ptr::null_mut();
            check((driver.cu_ctx_create)(&raw mut context, 0, device))?;
            check((driver.cu_ctx_set_current)(context))?;

            let ptx = CString::new(PTX).map_err(|_| CudaError::InvalidPtx)?;
            let mut module = ptr::null_mut();
            check((driver.cu_module_load_data_ex)(
                &raw mut module,
                ptx.as_ptr().cast(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            ))?;

            let kernel_name = c"brainstorm_search_kernel";
            let mut kernel = ptr::null_mut();
            check((driver.cu_module_get_function)(
                &raw mut kernel,
                module,
                kernel_name.as_ptr(),
            ))?;
            let debug_kernel_name = c"brainstorm_debug_seed_kernel";
            let mut debug_kernel = ptr::null_mut();
            check((driver.cu_module_get_function)(
                &raw mut debug_kernel,
                module,
                debug_kernel_name.as_ptr(),
            ))?;

            let mut d_params = 0;
            let mut d_best_offset = 0;
            let mut d_debug = 0;
            check((driver.cu_mem_alloc)(
                &raw mut d_params,
                mem::size_of::<CudaFilterParams>(),
            ))?;
            check((driver.cu_mem_alloc)(
                &raw mut d_best_offset,
                mem::size_of::<u64>(),
            ))?;
            check((driver.cu_mem_alloc)(
                &raw mut d_debug,
                mem::size_of::<[u64; 8]>(),
            ))?;

            Ok(Self {
                driver,
                context,
                module,
                kernel,
                debug_kernel,
                d_params,
                d_best_offset,
                d_debug,
            })
        }
    }

    fn search(
        &mut self,
        seed_start: &str,
        params: &CudaFilterParams,
        num_seeds: i64,
    ) -> Result<Option<String>, CudaError> {
        let mut remaining = num_seeds.clamp(0, SEED_SPACE);
        if remaining == 0 {
            return Ok(None);
        }
        let mut start_seed = Seed::from_str(seed_start).id();
        let launch_seeds = cuda_launch_seeds();

        unsafe {
            check((self.driver.cu_ctx_set_current)(self.context))?;
            check((self.driver.cu_memcpy_htod)(
                self.d_params,
                ptr::from_ref(params).cast(),
                mem::size_of::<CudaFilterParams>(),
            ))?;
        }

        while remaining > 0 {
            let count = remaining.min(launch_seeds);
            let found = self.search_launch(start_seed, count)?;
            if let Some(offset) = found {
                let id = (start_seed + i64::try_from(offset).unwrap_or(0)).rem_euclid(SEED_SPACE);
                return Ok(Some(Seed::from_id(id).to_string()));
            }
            remaining -= count;
            start_seed = (start_seed + count).rem_euclid(SEED_SPACE);
        }
        Ok(None)
    }

    fn search_launch(&mut self, start_seed: i64, count: i64) -> Result<Option<u64>, CudaError> {
        let mut best_offset = NO_RESULT;
        unsafe {
            check((self.driver.cu_memcpy_htod)(
                self.d_best_offset,
                ptr::from_ref(&best_offset).cast(),
                mem::size_of::<u64>(),
            ))?;

            let mut start_seed_arg = start_seed;
            let mut count_arg = count;
            let mut params_arg = self.d_params;
            let mut best_arg = self.d_best_offset;
            let mut args = [
                ptr::from_mut(&mut start_seed_arg).cast::<c_void>(),
                ptr::from_mut(&mut count_arg).cast::<c_void>(),
                ptr::from_mut(&mut params_arg).cast::<c_void>(),
                ptr::from_mut(&mut best_arg).cast::<c_void>(),
            ];

            check((self.driver.cu_launch_kernel)(
                self.kernel,
                GRID_SIZE,
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
            check((self.driver.cu_ctx_synchronize)())?;
            check((self.driver.cu_memcpy_dtoh)(
                ptr::from_mut(&mut best_offset).cast(),
                self.d_best_offset,
                mem::size_of::<u64>(),
            ))?;
        }

        if best_offset == NO_RESULT {
            Ok(None)
        } else {
            Ok(Some(best_offset))
        }
    }

    fn debug_seed(&mut self, seed_id: i64) -> Result<[u64; 8], CudaError> {
        let mut out = [0_u64; 8];
        unsafe {
            check((self.driver.cu_ctx_set_current)(self.context))?;
            let mut seed_arg = seed_id;
            let mut debug_arg = self.d_debug;
            let mut args = [
                ptr::from_mut(&mut seed_arg).cast::<c_void>(),
                ptr::from_mut(&mut debug_arg).cast::<c_void>(),
            ];
            check((self.driver.cu_launch_kernel)(
                self.debug_kernel,
                1,
                1,
                1,
                1,
                1,
                1,
                0,
                ptr::null_mut(),
                args.as_mut_ptr(),
                ptr::null_mut(),
            ))?;
            check((self.driver.cu_ctx_synchronize)())?;
            check((self.driver.cu_memcpy_dtoh)(
                out.as_mut_ptr().cast(),
                self.d_debug,
                mem::size_of::<[u64; 8]>(),
            ))?;
        }
        Ok(out)
    }
}

impl Drop for CudaEngine {
    fn drop(&mut self) {
        unsafe {
            let _ = (self.driver.cu_ctx_set_current)(self.context);
            if self.d_params != 0 {
                let _ = (self.driver.cu_mem_free)(self.d_params);
            }
            if self.d_best_offset != 0 {
                let _ = (self.driver.cu_mem_free)(self.d_best_offset);
            }
            if self.d_debug != 0 {
                let _ = (self.driver.cu_mem_free)(self.d_debug);
            }
            if !self.module.is_null() {
                let _ = (self.driver.cu_module_unload)(self.module);
            }
            if !self.context.is_null() {
                let _ = (self.driver.cu_ctx_destroy)(self.context);
            }
        }
    }
}

#[derive(Debug)]
enum CudaError {
    Load,
    Symbol,
    NoDevice,
    InvalidPtx,
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
    cu_ctx_set_current: extern_cuda_fn!(fn(CUcontext) -> CUresult),
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
    cu_ctx_synchronize: extern_cuda_fn!(fn() -> CUresult),
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
                cu_ctx_set_current: lib.symbol(b"cuCtxSetCurrent\0")?,
                cu_module_load_data_ex: lib.symbol(b"cuModuleLoadDataEx\0")?,
                cu_module_get_function: lib.symbol(b"cuModuleGetFunction\0")?,
                cu_module_unload: lib.symbol(b"cuModuleUnload\0")?,
                cu_mem_alloc: lib.symbol_any(&[b"cuMemAlloc_v2\0", b"cuMemAlloc\0"])?,
                cu_mem_free: lib.symbol_any(&[b"cuMemFree_v2\0", b"cuMemFree\0"])?,
                cu_memcpy_htod: lib.symbol_any(&[b"cuMemcpyHtoD_v2\0", b"cuMemcpyHtoD\0"])?,
                cu_memcpy_dtoh: lib.symbol_any(&[b"cuMemcpyDtoH_v2\0", b"cuMemcpyDtoH\0"])?,
                cu_launch_kernel: lib.symbol(b"cuLaunchKernel\0")?,
                cu_ctx_synchronize: lib.symbol(b"cuCtxSynchronize\0")?,
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
