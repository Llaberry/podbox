//! One line to file descriptor 2, without allocating and without `println!`.
//!
//! ⛔ [`TODO/interpose.md`](../../../TODO/interpose.md) T-0701 constraint 3.
//! `println!` locks stdout, allocates and formats through machinery that can
//! re-enter an interposed call; and stdout belongs to the payload
//! ([`TODO/milestones.md`](../../../TODO/milestones.md) T-1104), so a word from
//! podbox on it corrupts every pipeline the payload is in.
//!
//! ⚠ Everything here is a fixed stack buffer and one `write(2)`.

use core::ffi::{c_char, c_void};

extern "C" {
    fn write(fd: i32, buf: *const c_void, count: usize) -> isize;
    fn getenv(name: *const c_char) -> *mut c_char;
}

/// Every line this object prints starts here, so a payload's log can be grepped
/// for what podbox did inside it.
const PREFIX: &[u8] = b"podbox: interpose: ";

/// ⚠ Off by default. This object runs inside every process of a container,
/// including ones that spawn thousands of children, and a line per intercepted
/// call would bury the payload's own output. `PODBOX_INTERPOSE_DEBUG=1` turns
/// the per-call notes on; ⛔ a REFUSAL is printed either way, because a refusal
/// nobody can see is the `sandlock` failure mode.
pub fn debug() -> bool {
    let p = unsafe { getenv(c"PODBOX_INTERPOSE_DEBUG".as_ptr()) };
    !p.is_null() && unsafe { *p } != 0 && unsafe { *p } != b'0' as c_char
}

/// Write `parts`, joined, with the prefix and a newline. ⚠ Truncated rather
/// than allocated: 512 bytes is more than any message here, and a message that
/// did not fit is a message, not a panic.
pub fn line(parts: &[&[u8]]) {
    let mut buf = [0u8; 512];
    let mut n = 0usize;
    let mut push = |src: &[u8], n: &mut usize| {
        for b in src {
            if *n >= buf.len() - 1 {
                return;
            }
            buf[*n] = *b;
            *n += 1;
        }
    };
    push(PREFIX, &mut n);
    for p in parts {
        push(p, &mut n);
    }
    if n < buf.len() {
        buf[n] = b'\n';
        n += 1;
    }
    unsafe {
        let _ = write(2, buf.as_ptr() as *const c_void, n);
    }
}

/// A decimal `u64` in a caller-owned buffer, so a message can name a number
/// without `format!`.
pub struct Num([u8; 20], usize);

impl Num {
    pub fn new(mut v: u64) -> Num {
        let mut d = [0u8; 20];
        let mut i = d.len();
        loop {
            i -= 1;
            d[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        Num(d, i)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0[self.1..]
    }
}
