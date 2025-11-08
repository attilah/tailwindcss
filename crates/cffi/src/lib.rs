#![deny(unsafe_op_in_unsafe_fn)]
//! C-compatible bindings that mirror the Node.js Oxide integration.
//!
//! The exposed API surface matches the features offered through the Node bindings:
//! configuring sources, scanning files or inline content, resolving glob metadata,
//! and extracting candidates with byte offsets.

use std::ffi::{CStr, CString};
use std::mem;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::ptr;

use tailwindcss_oxide::{
    ChangedContent as OxideChangedContent, GlobEntry as OxideGlobEntry,
    PublicSourceEntry as OxidePublicSourceEntry, Scanner,
};

/// C-compatible representation of a Tailwind glob source entry.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TailwindSourceEntry {
    pub base: *const c_char,
    pub pattern: *const c_char,
    pub negated: u8,
}

/// Identifies the kind of changed content passed to the scanner.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TailwindChangedContentKind {
    File = 0,
    Content = 1,
}

/// C-compatible representation of changed content.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TailwindChangedContent {
    pub kind: TailwindChangedContentKind,
    pub data: *const c_char,
    pub extension: *const c_char,
}

/// Array of C strings returned by the library.
#[repr(C)]
pub struct TailwindStringList {
    pub data: *mut *mut c_char,
    pub len: usize,
    pub capacity: usize,
}

impl Default for TailwindStringList {
    fn default() -> Self {
        Self {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }
}

/// Candidate string with its byte position inside the source content.
#[repr(C)]
pub struct TailwindCandidateWithPosition {
    pub candidate: *mut c_char,
    pub position: usize,
}

/// Array of candidate strings with positions.
#[repr(C)]
pub struct TailwindCandidateList {
    pub data: *mut TailwindCandidateWithPosition,
    pub len: usize,
    pub capacity: usize,
}

impl Default for TailwindCandidateList {
    fn default() -> Self {
        Self {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }
}

/// Glob entry returned by the scanner.
#[repr(C)]
pub struct TailwindGlobEntry {
    pub base: *mut c_char,
    pub pattern: *mut c_char,
}

/// Array of glob entries.
#[repr(C)]
pub struct TailwindGlobEntryList {
    pub data: *mut TailwindGlobEntry,
    pub len: usize,
    pub capacity: usize,
}

impl Default for TailwindGlobEntryList {
    fn default() -> Self {
        Self {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }
}

/// Opaque scanner handle exposed to C.
#[repr(C)]
pub struct TailwindScanner {
    _private: [u8; 0],
}

struct ScannerWrapper {
    inner: Scanner,
}

unsafe fn c_str_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    // SAFETY: Caller guarantees `ptr` points to a valid, nul-terminated UTF-8 string.
    let c_str = unsafe { CStr::from_ptr(ptr) };
    Some(c_str.to_string_lossy().into_owned())
}

fn build_sources(ptr: *const TailwindSourceEntry, len: usize) -> Vec<OxidePublicSourceEntry> {
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }

    // SAFETY: Caller guarantees that `ptr` points to `len` valid entries.
    let entries = unsafe { std::slice::from_raw_parts(ptr, len) };

    entries
        .iter()
        .filter_map(|entry| {
            let base = unsafe { c_str_to_string(entry.base)? };
            let pattern =
                unsafe { c_str_to_string(entry.pattern) }.unwrap_or_else(|| "**/*".to_string());

            Some(OxidePublicSourceEntry {
                base,
                pattern,
                negated: entry.negated != 0,
            })
        })
        .collect()
}

fn build_changed_content(
    ptr: *const TailwindChangedContent,
    len: usize,
) -> Vec<OxideChangedContent> {
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }

    // SAFETY: Caller guarantees that `ptr` points to `len` valid entries.
    let entries = unsafe { std::slice::from_raw_parts(ptr, len) };

    entries.iter().filter_map(convert_changed_content).collect()
}

fn convert_changed_content(entry: &TailwindChangedContent) -> Option<OxideChangedContent> {
    let extension = unsafe { c_str_to_string(entry.extension) }.unwrap_or_default();

    match entry.kind {
        TailwindChangedContentKind::File => {
            let path = unsafe { c_str_to_string(entry.data)? };
            Some(OxideChangedContent::File(PathBuf::from(path), extension))
        }
        TailwindChangedContentKind::Content => {
            let content = unsafe { c_str_to_string(entry.data) }.unwrap_or_default();
            Some(OxideChangedContent::Content(content, extension))
        }
    }
}

fn to_string_list(strings: Vec<String>) -> TailwindStringList {
    if strings.is_empty() {
        return TailwindStringList::default();
    }

    let mut data: Vec<*mut c_char> = Vec::with_capacity(strings.len());

    for string in strings {
        match CString::new(string) {
            Ok(c_string) => data.push(c_string.into_raw()),
            Err(_) => continue,
        }
    }

    if data.is_empty() {
        return TailwindStringList::default();
    }

    let len = data.len();
    let capacity = data.capacity();
    let pointer = data.as_mut_ptr();
    mem::forget(data);

    TailwindStringList {
        data: pointer,
        len,
        capacity,
    }
}

fn to_candidate_list(items: Vec<(String, usize)>) -> TailwindCandidateList {
    if items.is_empty() {
        return TailwindCandidateList::default();
    }

    let mut data: Vec<TailwindCandidateWithPosition> = Vec::with_capacity(items.len());

    for (candidate, position) in items {
        match CString::new(candidate) {
            Ok(c_string) => data.push(TailwindCandidateWithPosition {
                candidate: c_string.into_raw(),
                position,
            }),
            Err(_) => continue,
        }
    }

    if data.is_empty() {
        return TailwindCandidateList::default();
    }

    let len = data.len();
    let capacity = data.capacity();
    let pointer = data.as_mut_ptr();
    mem::forget(data);

    TailwindCandidateList {
        data: pointer,
        len,
        capacity,
    }
}

fn to_glob_entry_list(entries: Vec<OxideGlobEntry>) -> TailwindGlobEntryList {
    if entries.is_empty() {
        return TailwindGlobEntryList::default();
    }

    let mut data: Vec<TailwindGlobEntry> = Vec::with_capacity(entries.len());

    for entry in entries {
        let base = match CString::new(entry.base) {
            Ok(inner) => inner.into_raw(),
            Err(_) => continue,
        };

        let pattern = match CString::new(entry.pattern) {
            Ok(inner) => inner.into_raw(),
            Err(_) => {
                unsafe {
                    let _ = CString::from_raw(base);
                }
                continue;
            }
        };

        data.push(TailwindGlobEntry { base, pattern });
    }

    if data.is_empty() {
        return TailwindGlobEntryList::default();
    }

    let len = data.len();
    let capacity = data.capacity();
    let pointer = data.as_mut_ptr();
    mem::forget(data);

    TailwindGlobEntryList {
        data: pointer,
        len,
        capacity,
    }
}

unsafe fn scanner_wrapper_mut<'a>(scanner: *mut TailwindScanner) -> Option<&'a mut ScannerWrapper> {
    if scanner.is_null() {
        return None;
    }

    // SAFETY: `scanner` must point to a valid `ScannerWrapper` allocated by `tailwindcss_scanner_new`.
    Some(unsafe { &mut *(scanner as *mut ScannerWrapper) })
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_scanner_new(
    sources: *const TailwindSourceEntry,
    sources_len: usize,
) -> *mut TailwindScanner {
    let public_sources = build_sources(sources, sources_len);
    let wrapper = ScannerWrapper {
        inner: Scanner::new(public_sources),
    };

    Box::into_raw(Box::new(wrapper)) as *mut TailwindScanner
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_scanner_free(scanner: *mut TailwindScanner) {
    if scanner.is_null() {
        return;
    }

    // SAFETY: `scanner` originated from `tailwindcss_scanner_new`.
    unsafe {
        drop(Box::from_raw(scanner as *mut ScannerWrapper));
    }
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_scanner_scan(
    scanner: *mut TailwindScanner,
) -> TailwindStringList {
    let Some(scanner) = (unsafe { scanner_wrapper_mut(scanner) }) else {
        return TailwindStringList::default();
    };

    to_string_list(scanner.inner.scan())
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_scanner_scan_files(
    scanner: *mut TailwindScanner,
    changed_content: *const TailwindChangedContent,
    changed_content_len: usize,
) -> TailwindStringList {
    let Some(scanner) = (unsafe { scanner_wrapper_mut(scanner) }) else {
        return TailwindStringList::default();
    };

    let changed = build_changed_content(changed_content, changed_content_len);

    to_string_list(scanner.inner.scan_content(changed))
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_scanner_get_candidates_with_positions(
    scanner: *mut TailwindScanner,
    content: TailwindChangedContent,
) -> TailwindCandidateList {
    let Some(scanner) = (unsafe { scanner_wrapper_mut(scanner) }) else {
        return TailwindCandidateList::default();
    };

    let Some(changed) = convert_changed_content(&content) else {
        return TailwindCandidateList::default();
    };

    to_candidate_list(scanner.inner.get_candidates_with_positions(changed))
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_scanner_get_files(
    scanner: *mut TailwindScanner,
) -> TailwindStringList {
    let Some(scanner) = (unsafe { scanner_wrapper_mut(scanner) }) else {
        return TailwindStringList::default();
    };

    to_string_list(scanner.inner.get_files())
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_scanner_get_globs(
    scanner: *mut TailwindScanner,
) -> TailwindGlobEntryList {
    let Some(scanner) = (unsafe { scanner_wrapper_mut(scanner) }) else {
        return TailwindGlobEntryList::default();
    };

    to_glob_entry_list(scanner.inner.get_globs())
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_scanner_get_normalized_sources(
    scanner: *mut TailwindScanner,
) -> TailwindGlobEntryList {
    let Some(scanner) = (unsafe { scanner_wrapper_mut(scanner) }) else {
        return TailwindGlobEntryList::default();
    };

    to_glob_entry_list(scanner.inner.get_normalized_sources())
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_string_list_free(list: TailwindStringList) {
    if list.data.is_null() {
        return;
    }

    // SAFETY: `list` originates from `to_string_list`, so the slice is valid.
    let data = unsafe { Vec::from_raw_parts(list.data, list.len, list.capacity) };
    for ptr in data {
        if !ptr.is_null() {
            unsafe {
                let _ = CString::from_raw(ptr);
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_candidate_list_free(list: TailwindCandidateList) {
    if list.data.is_null() {
        return;
    }

    // SAFETY: `list` originates from `to_candidate_list`.
    let data = unsafe { Vec::from_raw_parts(list.data, list.len, list.capacity) };
    for entry in data {
        if !entry.candidate.is_null() {
            unsafe {
                let _ = CString::from_raw(entry.candidate);
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tailwindcss_glob_list_free(list: TailwindGlobEntryList) {
    if list.data.is_null() {
        return;
    }

    // SAFETY: `list` originates from `to_glob_entry_list`.
    let data = unsafe { Vec::from_raw_parts(list.data, list.len, list.capacity) };
    for entry in data {
        if !entry.base.is_null() {
            unsafe {
                let _ = CString::from_raw(entry.base);
            }
        }
        if !entry.pattern.is_null() {
            unsafe {
                let _ = CString::from_raw(entry.pattern);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    unsafe fn string_list_to_vec(list: TailwindStringList) -> Vec<String> {
        if list.data.is_null() || list.len == 0 {
            unsafe {
                tailwindcss_string_list_free(list);
            }
            return Vec::new();
        }

        // SAFETY: `data` originates from `to_string_list`.
        let slice = unsafe { std::slice::from_raw_parts(list.data, list.len) };
        let mut result = Vec::with_capacity(list.len);

        for &item in slice {
            if item.is_null() {
                continue;
            }
            let c_str = unsafe { CStr::from_ptr(item) };
            result.push(c_str.to_string_lossy().into_owned());
        }

        unsafe {
            tailwindcss_string_list_free(list);
        }

        result
    }

    unsafe fn candidate_list_to_vec(list: TailwindCandidateList) -> Vec<(String, usize)> {
        if list.data.is_null() || list.len == 0 {
            unsafe {
                tailwindcss_candidate_list_free(list);
            }
            return Vec::new();
        }

        // SAFETY: `data` originates from `to_candidate_list`.
        let slice = unsafe { std::slice::from_raw_parts(list.data, list.len) };
        let mut result = Vec::with_capacity(list.len);

        for entry in slice {
            if entry.candidate.is_null() {
                continue;
            }
            let candidate = unsafe { CStr::from_ptr(entry.candidate) };
            result.push((candidate.to_string_lossy().into_owned(), entry.position));
        }

        unsafe {
            tailwindcss_candidate_list_free(list);
        }

        result
    }

    unsafe fn glob_list_to_vec(list: TailwindGlobEntryList) -> Vec<(String, String)> {
        if list.data.is_null() || list.len == 0 {
            unsafe {
                tailwindcss_glob_list_free(list);
            }
            return Vec::new();
        }

        // SAFETY: `data` originates from `to_glob_entry_list`.
        let slice = unsafe { std::slice::from_raw_parts(list.data, list.len) };
        let mut result = Vec::with_capacity(list.len);

        for entry in slice {
            let base = if entry.base.is_null() {
                String::new()
            } else {
                let base = unsafe { CStr::from_ptr(entry.base) };
                base.to_string_lossy().into_owned()
            };

            let pattern = if entry.pattern.is_null() {
                String::new()
            } else {
                let pattern = unsafe { CStr::from_ptr(entry.pattern) };
                pattern.to_string_lossy().into_owned()
            };

            result.push((base, pattern));
        }

        unsafe {
            tailwindcss_glob_list_free(list);
        }

        result
    }

    #[test]
    fn scan_reads_initial_sources() {
        let dir = tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("index.html");
        fs::write(
            &file_path,
            r#"<div class="text-lg font-bold underline"></div>"#,
        )
        .expect("failed to write file");

        let base = CString::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let pattern = CString::new("**/*").unwrap();

        let source = TailwindSourceEntry {
            base: base.as_ptr(),
            pattern: pattern.as_ptr(),
            negated: 0,
        };

        let scanner = unsafe { tailwindcss_scanner_new(&source, 1) };

        let candidates = unsafe { string_list_to_vec(tailwindcss_scanner_scan(scanner)) };
        assert!(
            candidates.iter().any(|candidate| candidate == "font-bold"),
            "Expected font-bold in {candidates:?}"
        );

        let files = unsafe { string_list_to_vec(tailwindcss_scanner_get_files(scanner)) };
        assert_eq!(files.len(), 1);

        let globs = unsafe { glob_list_to_vec(tailwindcss_scanner_get_globs(scanner)) };
        assert!(!globs.is_empty());

        unsafe {
            tailwindcss_scanner_free(scanner);
        }
    }

    #[test]
    fn scan_files_reads_from_disk() {
        let dir = tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("snippet.html");
        fs::write(&file_path, r#"<section class="md:flex text-sm"></section>"#)
            .expect("failed to write file");

        let base = CString::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let pattern = CString::new("**/*.html").unwrap();

        let source = TailwindSourceEntry {
            base: base.as_ptr(),
            pattern: pattern.as_ptr(),
            negated: 0,
        };

        let scanner = unsafe { tailwindcss_scanner_new(&source, 1) };

        let path = CString::new(file_path.to_string_lossy().into_owned()).unwrap();
        let extension = CString::new("html").unwrap();

        let changed = TailwindChangedContent {
            kind: TailwindChangedContentKind::File,
            data: path.as_ptr(),
            extension: extension.as_ptr(),
        };

        let candidates =
            unsafe { string_list_to_vec(tailwindcss_scanner_scan_files(scanner, &changed, 1)) };

        assert!(
            candidates.iter().any(|candidate| candidate == "md:flex"),
            "Expected md:flex in {candidates:?}"
        );

        unsafe {
            tailwindcss_scanner_free(scanner);
        }
    }

    #[test]
    fn get_candidates_accepts_inline_content() {
        let scanner = unsafe { tailwindcss_scanner_new(ptr::null(), 0) };

        let content = CString::new(r#"<span class="font-semibold"></span>"#).unwrap();
        let extension = CString::new("html").unwrap();

        let changed = TailwindChangedContent {
            kind: TailwindChangedContentKind::Content,
            data: content.as_ptr(),
            extension: extension.as_ptr(),
        };

        let candidates = unsafe {
            candidate_list_to_vec(tailwindcss_scanner_get_candidates_with_positions(
                scanner, changed,
            ))
        };

        assert!(
            candidates
                .iter()
                .any(|(candidate, _)| candidate == "font-semibold"),
            "Expected font-semibold in {candidates:?}"
        );

        unsafe {
            tailwindcss_scanner_free(scanner);
        }
    }

    #[test]
    fn get_candidates_from_file_content() {
        let dir = tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("inline.html");
        let file_contents = r#"<div class="font-black text-sm"></div>"#;
        fs::write(&file_path, file_contents).expect("failed to write file");

        let base = CString::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let pattern = CString::new("**/*.html").unwrap();

        let source = TailwindSourceEntry {
            base: base.as_ptr(),
            pattern: pattern.as_ptr(),
            negated: 0,
        };

        let scanner = unsafe { tailwindcss_scanner_new(&source, 1) };

        let path = CString::new(file_path.to_string_lossy().into_owned()).unwrap();
        let extension = CString::new("html").unwrap();

        let changed = TailwindChangedContent {
            kind: TailwindChangedContentKind::File,
            data: path.as_ptr(),
            extension: extension.as_ptr(),
        };

        let candidates = unsafe {
            candidate_list_to_vec(tailwindcss_scanner_get_candidates_with_positions(
                scanner, changed,
            ))
        };

        let expected_offset = file_contents.find("font-black").unwrap();
        let match_entry = candidates
            .iter()
            .find(|(candidate, _)| candidate == "font-black")
            .cloned();

        assert!(
            match_entry.is_some(),
            "Expected font-black candidate in {candidates:?}"
        );

        assert_eq!(
            match_entry.unwrap().1,
            expected_offset,
            "Expected byte position {expected_offset}"
        );

        unsafe {
            tailwindcss_scanner_free(scanner);
        }
    }

    #[test]
    fn files_return_canonical_paths() {
        let dir = tempdir().expect("failed to create temp dir");
        let nested = dir.path().join("nested");
        fs::create_dir(&nested).expect("failed to create nested dir");
        let file_path = nested.join("index.html");
        fs::write(&file_path, r#"<span class="italic"></span>"#).expect("failed to write file");

        let base = CString::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let pattern = CString::new("**/*.html").unwrap();

        let source = TailwindSourceEntry {
            base: base.as_ptr(),
            pattern: pattern.as_ptr(),
            negated: 0,
        };

        let scanner = unsafe { tailwindcss_scanner_new(&source, 1) };
        let files = unsafe { string_list_to_vec(tailwindcss_scanner_get_files(scanner)) };

        assert!(
            files
                .iter()
                .any(|entry| entry.ends_with("nested/index.html")),
            "Expected nested/index.html in {files:?}"
        );
        assert!(
            files.iter().all(|entry| Path::new(entry).is_absolute()),
            "Expected absolute file paths"
        );

        unsafe {
            tailwindcss_scanner_free(scanner);
        }
    }

    #[test]
    fn globs_and_normalized_sources_are_reported() {
        let dir = tempdir().expect("failed to create temp dir");
        let base = CString::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let pattern_all = CString::new("**/*").unwrap();
        let pattern_html = CString::new("**/*.html").unwrap();

        let sources = [
            TailwindSourceEntry {
                base: base.as_ptr(),
                pattern: pattern_all.as_ptr(),
                negated: 0,
            },
            TailwindSourceEntry {
                base: base.as_ptr(),
                pattern: pattern_html.as_ptr(),
                negated: 0,
            },
        ];

        let scanner = unsafe { tailwindcss_scanner_new(sources.as_ptr(), sources.len()) };

        let globs = unsafe { glob_list_to_vec(tailwindcss_scanner_get_globs(scanner)) };
        assert!(!globs.is_empty(), "Expected to receive glob entries");

        let normalized =
            unsafe { glob_list_to_vec(tailwindcss_scanner_get_normalized_sources(scanner)) };
        assert!(
            normalized
                .iter()
                .any(|(_, pattern)| pattern == "**/*" || pattern == "**/*.html"),
            "Expected normalized sources to contain auto or pattern entries: {normalized:?}"
        );

        unsafe {
            tailwindcss_scanner_free(scanner);
        }
    }

    #[test]
    fn scan_files_accepts_multiple_inputs() {
        let scanner = unsafe { tailwindcss_scanner_new(ptr::null(), 0) };

        let html = CString::new(r#"<div class="text-xl"></div>"#).unwrap();
        let html_ext = CString::new("html").unwrap();
        let rust = CString::new(r#"fn main() { println!("class=\"font-thin\""); }"#).unwrap();
        let rust_ext = CString::new("rs").unwrap();

        let entries = [
            TailwindChangedContent {
                kind: TailwindChangedContentKind::Content,
                data: html.as_ptr(),
                extension: html_ext.as_ptr(),
            },
            TailwindChangedContent {
                kind: TailwindChangedContentKind::Content,
                data: rust.as_ptr(),
                extension: rust_ext.as_ptr(),
            },
        ];

        let candidates = unsafe {
            string_list_to_vec(tailwindcss_scanner_scan_files(
                scanner,
                entries.as_ptr(),
                entries.len(),
            ))
        };

        assert!(
            candidates.contains(&"text-xl".to_string()),
            "Expected text-xl in {candidates:?}"
        );
        assert!(
            candidates.contains(&"font-thin".to_string()),
            "Expected font-thin in {candidates:?}"
        );

        unsafe {
            tailwindcss_scanner_free(scanner);
        }
    }
}
