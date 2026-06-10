#![allow(unsafe_code)]

use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_longlong};
use std::ptr;

use crate::brainstorm_search_core;
use crate::engine::cuda;
use crate::filters::FilterConfig;

/// Searches for the earliest matching seed.
///
/// # Safety
///
/// Every non-null string pointer must remain valid, immutable, and
/// NUL-terminated for the duration of this call. A non-null return value must
/// be released exactly once with [`free_result`].
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn brainstorm_search(
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
    cuda::reset_last_search_used();
    let search = || {
        // SAFETY: the caller upholds every input pointer's validity for this call.
        let (
            seed_start,
            voucher_key,
            pack_key,
            tag1_key,
            tag2_key,
            joker_name,
            joker_location,
            deck_key,
        ) = unsafe {
            (
                c_string_lossy(seed_start),
                c_string_lossy(voucher_key),
                c_string_lossy(pack_key),
                c_string_lossy(tag1_key),
                c_string_lossy(tag2_key),
                c_string_lossy(joker_name),
                c_string_lossy(joker_location),
                c_string_lossy(deck_key),
            )
        };

        brainstorm_search_impl(
            &seed_start,
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
            num_seeds,
            threads,
        )
    };

    #[cfg(panic = "unwind")]
    let result = std::panic::catch_unwind(search).ok().flatten();
    #[cfg(panic = "abort")]
    let result = search();

    result.map_or_else(ptr::null_mut, CString::into_raw)
}

#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn brainstorm_search_impl(
    seed_start: &str,
    voucher_key: &str,
    pack_key: &str,
    tag1_key: &str,
    tag2_key: &str,
    joker_name: &str,
    joker_location: &str,
    souls: c_double,
    observatory: bool,
    perkeo: bool,
    deck_key: &str,
    erratic: bool,
    no_faces: bool,
    min_face_cards: c_int,
    suit_ratio: c_double,
    num_seeds: c_longlong,
    threads: c_int,
) -> Option<CString> {
    let cfg = FilterConfig::from_raw(
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
    );

    let result = brainstorm_search_core(seed_start, &cfg, num_seeds, threads)?;
    if result.is_empty() {
        return None;
    }
    CString::new(result).ok()
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn immolate_set_log_path(_path: *const c_char) {}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn immolate_set_cuda_enabled(enabled: bool) {
    cuda::set_cuda_enabled(enabled);
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn immolate_last_search_used_cuda() -> bool {
    cuda::last_search_used()
}

/// Releases a result returned by [`brainstorm_search`].
///
/// # Safety
///
/// `result` must be null or an unfreed pointer returned by [`brainstorm_search`].
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn free_result(result: *mut c_char) {
    if result.is_null() {
        return;
    }
    // SAFETY: non-null results come from this DLL's `CString::into_raw` export path.
    unsafe {
        drop(CString::from_raw(result));
    }
}

/// Borrows valid UTF-8 C strings and allocates only when replacement is needed.
///
/// # Safety
///
/// A non-null `ptr` must reference an immutable NUL-terminated byte sequence
/// that remains valid for the returned borrow's lifetime.
unsafe fn c_string_lossy<'a>(ptr: *const c_char) -> Cow<'a, str> {
    if ptr.is_null() {
        return Cow::Borrowed("");
    }
    // SAFETY: callers provide a live NUL-terminated C string for every non-null argument.
    unsafe { CStr::from_ptr(ptr) }.to_string_lossy()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_c_string_is_borrowed_empty() {
        // SAFETY: null is explicitly supported and never dereferenced.
        let value = unsafe { c_string_lossy(ptr::null()) };
        assert!(matches!(value, Cow::Borrowed("")));
    }

    #[test]
    fn valid_utf8_c_string_is_borrowed() {
        // SAFETY: the static C literal remains valid and immutable.
        let value = unsafe { c_string_lossy(c"KRVH".as_ptr()) };
        assert!(matches!(value, Cow::Borrowed("KRVH")));
    }

    #[test]
    fn invalid_utf8_c_string_is_replaced_in_owned_storage() {
        // SAFETY: the static C literal remains valid and immutable.
        let value = unsafe { c_string_lossy(c"K\xff".as_ptr()) };
        assert!(matches!(value, Cow::Owned(value) if value == "K\u{fffd}"));
    }

    #[test]
    fn cuda_status_export_reports_the_current_threads_marker() {
        cuda::reset_last_search_used();
        assert!(!immolate_last_search_used_cuda());

        cuda::mark_last_search_used();
        assert!(immolate_last_search_used_cuda());

        cuda::reset_last_search_used();
    }
}
