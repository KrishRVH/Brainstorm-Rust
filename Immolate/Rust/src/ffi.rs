#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_longlong};
use std::ptr;
use std::sync::Mutex;

use crate::brainstorm_search_core;
use crate::engine::cuda;
use crate::filters::FilterConfig;

static LAST_ERROR: Mutex<Option<CString>> = Mutex::new(None);

#[unsafe(no_mangle)]
pub extern "C" fn brainstorm_search(
    seed_start: *const c_char,
    voucher_key: *const c_char,
    pack_key: *const c_char,
    tag1_key: *const c_char,
    tag2_key: *const c_char,
    joker_name: *const c_char,
    joker_location: *const c_char,
    souls: c_double,
    observatory: bool,
    perkeo: bool,
    deck_key: *const c_char,
    erratic: bool,
    no_faces: bool,
    min_face_cards: c_int,
    suit_ratio: c_double,
    num_seeds: c_longlong,
    threads: c_int,
) -> *mut c_char {
    match std::panic::catch_unwind(|| {
        brainstorm_search_impl(
            seed_start,
            voucher_key,
            pack_key,
            tag1_key,
            tag2_key,
            joker_name,
            joker_location,
            souls,
            observatory,
            perkeo,
            deck_key,
            erratic,
            no_faces,
            min_face_cards,
            suit_ratio,
            num_seeds,
            threads,
        )
    }) {
        Ok(Ok(Some(result))) => {
            clear_last_error();
            result.into_raw()
        },
        Ok(Ok(None)) => {
            clear_last_error();
            ptr::null_mut()
        },
        Ok(Err(message)) => {
            set_last_error(message);
            ptr::null_mut()
        },
        Err(_) => {
            set_last_error("brainstorm_search panicked");
            ptr::null_mut()
        },
    }
}

fn brainstorm_search_impl(
    seed_start: *const c_char,
    voucher_key: *const c_char,
    pack_key: *const c_char,
    tag1_key: *const c_char,
    tag2_key: *const c_char,
    joker_name: *const c_char,
    joker_location: *const c_char,
    souls: c_double,
    observatory: bool,
    perkeo: bool,
    deck_key: *const c_char,
    erratic: bool,
    no_faces: bool,
    min_face_cards: c_int,
    suit_ratio: c_double,
    num_seeds: c_longlong,
    threads: c_int,
) -> Result<Option<CString>, String> {
    let seed_start = c_string_lossy(seed_start);
    let voucher_key = c_string_lossy(voucher_key);
    let pack_key = c_string_lossy(pack_key);
    let tag1_key = c_string_lossy(tag1_key);
    let tag2_key = c_string_lossy(tag2_key);
    let joker_name = c_string_lossy(joker_name);
    let joker_location = c_string_lossy(joker_location);
    let deck_key = c_string_lossy(deck_key);

    let cfg = FilterConfig::from_raw(
        &voucher_key,
        &pack_key,
        &tag1_key,
        &tag2_key,
        &joker_name,
        &joker_location,
        souls,
        observatory,
        perkeo,
        &deck_key,
        erratic,
        no_faces,
        min_face_cards,
        suit_ratio,
    );

    let Some(result) = brainstorm_search_core(&seed_start, &cfg, num_seeds, threads) else {
        return Ok(None);
    };
    if result.is_empty() {
        return Ok(None);
    }
    CString::new(result)
        .map(Some)
        .map_err(|_| "search result contained an interior NUL byte".to_owned())
}

#[unsafe(no_mangle)]
pub extern "C" fn immolate_set_log_path(_path: *const c_char) {}

#[unsafe(no_mangle)]
pub extern "C" fn immolate_set_cuda_enabled(enabled: bool) {
    cuda::set_cuda_enabled(enabled);
}

#[unsafe(no_mangle)]
pub extern "C" fn immolate_last_error() -> *const c_char {
    LAST_ERROR
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|message| message.as_ptr()))
        .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn free_result(result: *mut c_char) {
    if result.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(result));
    }
}

fn c_string_lossy(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

fn set_last_error(message: impl Into<String>) {
    if let Ok(mut guard) = LAST_ERROR.lock() {
        *guard = CString::new(message.into()).ok();
    }
}

fn clear_last_error() {
    if let Ok(mut guard) = LAST_ERROR.lock() {
        *guard = None;
    }
}
