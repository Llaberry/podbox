//! `dlsym(RTLD_NEXT, ...)`, resolved once per entry point and cached.
//!
//! ⭐ [`TODO/interpose.md`](../../../TODO/interpose.md) T-0701's Decision. The
//! alternative -- resolving on every call -- is a `dlsym` inside every
//! intercepted `open`, and the corpus paid for it twice:
//! `references/fritzw__ld-preload-open`'s tracker carries "Cache the results of
//! dlsym" as pull request #3 and "fix memory corruption" as #4. The shape to
//! copy is `references/VHSgunzo__pathmap/tree/path-mapping.c:603-606`.
//!
//! ⛔ **No allocation and no lock on this path.** The cache is one
//! `AtomicPtr` per entry point, published with `Release` and read with
//! `Acquire`. Two threads racing resolve the same symbol twice and store the
//! same pointer, which costs a `dlsym` and is correct; a lock here could be
//! re-entered by the loader itself, and T-0701 forbids exactly that.
//!
//! ⚠ **`dlsym` is the one import that decides which payloads this object can
//! serve.** glibc 2.34 merged `libdl` into `libc`, so an object built against
//! 2.39 imports `dlsym@GLIBC_2.34` and the loader refuses it on a 2.31 payload.
//! That is not a defect to work around here: it is what
//! [`podbox_enter::abi`](../../../crates/podbox-enter/src/abi.rs) reads, and
//! `experiments/80-interposer-abi.sh` check E asserts podbox refuses the pair
//! before anything is loaded rather than after.

use core::ffi::{c_char, c_void};
use core::sync::atomic::{AtomicPtr, Ordering};

extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

/// `RTLD_NEXT`, which is `((void *) -1)` in both glibc's and musl's `dlfcn.h`.
const RTLD_NEXT: *mut c_void = usize::MAX as *mut c_void;

/// One cached pointer to the next definition of a symbol.
pub struct Next(AtomicPtr<c_void>);

impl Default for Next {
    fn default() -> Next {
        Next::new()
    }
}

impl Next {
    pub const fn new() -> Next {
        Next(AtomicPtr::new(core::ptr::null_mut()))
    }

    /// The real function, or null where the payload's libc does not define it.
    ///
    /// # Safety
    /// `name` must be a NUL-terminated C string, and the caller must transmute
    /// the result to the signature the symbol actually has.
    pub unsafe fn get(&self, name: &[u8]) -> *mut c_void {
        let cached = self.0.load(Ordering::Acquire);
        if !cached.is_null() {
            return cached;
        }
        debug_assert!(
            name.last() == Some(&0),
            "a dlsym name must be NUL-terminated"
        );
        let p = unsafe { dlsym(RTLD_NEXT, name.as_ptr() as *const c_char) };
        // ⚠ Stored even when null is not possible to distinguish from "not
        // resolved yet": a null is NOT cached, so a symbol the loader could not
        // find is retried rather than remembered as absent. That costs a dlsym
        // per call on a payload whose libc lacks the symbol, which is the case
        // podbox refuses before loading anyway.
        if !p.is_null() {
            self.0.store(p, Ordering::Release);
        }
        p
    }
}

/// Declare one interposed entry point.
///
/// ⛔ A macro rather than a copied body, which is
/// `references/VHSgunzo__pathmap/tree/path-mapping.c:250-262`'s own decision:
/// a new entry point is one line, and the forwarding cannot be got subtly wrong
/// in the twentieth copy.
#[macro_export]
macro_rules! real {
    ($vis:vis fn $binding:ident = $sym:literal ( $($arg:ty),* $(,)? ) -> $ret:ty) => {
        /// The payload's own definition, resolved on first use.
        ///
        /// ⚠ The BINDING and the SYMBOL are named separately, because the
        /// exported entry point in `lib.rs` has the symbol's own name: one
        /// identifier for both would be podbox resolving `chown` to itself.
        $vis fn $binding() -> Option<unsafe extern "C" fn($($arg),*) -> $ret> {
            static NEXT: $crate::real::Next = $crate::real::Next::new();
            let p = unsafe { NEXT.get(concat!($sym, "\0").as_bytes()) };
            if p.is_null() {
                None
            } else {
                // ⚠ The transmute is the whole point of this module and is why
                // every use of it names the signature at the call site.
                Some(unsafe { core::mem::transmute::<
                    *mut core::ffi::c_void,
                    unsafe extern "C" fn($($arg),*) -> $ret,
                >(p) })
            }
        }
    };
}
