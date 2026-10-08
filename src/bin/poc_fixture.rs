//! Governed-process test fixture for the filesystem-enforcement PoC.
//!
//! Deliberately plain, uninstrumented code: it does not know about
//! Pollux, the interposer, or Core at all, and does not link
//! `pollux-fs-interpose-poc` or `pollux-polyglot-native-bridge`. It just
//! calls the real libc `open()` on each path given on the command line
//! and reports what happened — exactly what any real, unmodified
//! application would do. Whatever gates that `open()` call is
//! `DYLD_INSERT_LIBRARIES` alone, injected from *outside* this binary by
//! the test harness — not requested by this binary itself.

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

extern "C" {
    fn open(path: *const c_char, flags: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    fn close(fd: c_int) -> c_int;
}

const O_RDONLY: c_int = 0;

fn main() {
    let mut exit_code = 0i32;

    for path in std::env::args().skip(1) {
        let cpath = CString::new(path.clone()).expect("path must not contain a NUL byte");

        // SAFETY: `cpath` is a valid, NUL-terminated C string for the
        // duration of this call.
        let fd = unsafe { open(cpath.as_ptr(), O_RDONLY) };

        if fd >= 0 {
            let mut buf = [0u8; 512];
            // SAFETY: `fd` was just opened successfully above and is not
            // used anywhere else concurrently; `buf` is a valid,
            // correctly-sized buffer for the duration of the call.
            let n = unsafe { read(fd, buf.as_mut_ptr() as *mut c_void, buf.len()) };
            // SAFETY: `fd` is still the same, still-open descriptor.
            unsafe { close(fd) };

            let n = n.max(0) as usize;
            let text = String::from_utf8_lossy(&buf[..n]);
            println!("OPEN_OK {path} bytes={n} content={text:?}");
        } else {
            let errno = std::io::Error::last_os_error();
            println!("OPEN_ERR {path} errno={errno:?}");
            exit_code = 1;
        }
    }

    std::process::exit(exit_code);
}
