
use crate::ffi::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::mem::transmute;

struct SpinLock {
    locked: AtomicBool,
}

#[cfg(target_os = "windows")]
#[inline]
fn asm_pause() { unsafe { asm!("pause"::::); }}

#[cfg(not(target_os = "windows"))]
#[inline]
fn asm_pause() {}

impl SpinLock {
    pub const fn new() -> SpinLock {
        SpinLock { locked: AtomicBool::new(false) }
    }

    pub fn lock(&mut self) {
        while !self.locked.compare_and_swap(false, true, Ordering::Relaxed) {
            asm_pause();
        }
    }

    pub fn unlock(&mut self) {
        self.locked.store(false, Ordering::Relaxed);
    }
}

#[repr(C)]
struct Extra {
    spin: SpinLock,
}

#[inline]
fn get_extra(l: *mut lua_State) -> &'static mut Extra {
    unsafe { transmute(lua_getextraspace(l)) }
}

#[no_mangle]
extern "C" fn ulua_lock(l: *mut lua_State) {
    get_extra(l).spin.lock();
}

#[no_mangle]
extern "C" fn ulua_unlock(l: *mut lua_State) {
    get_extra(l).spin.unlock();
}

#[no_mangle]
extern "C" fn ulua_init_lock(l: *mut lua_State) {
    *get_extra(l) = Extra { spin: SpinLock::new() };
}