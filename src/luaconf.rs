
use std::mem::size_of;
use libc::c_int;

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
pub const LUAI_MAXSTACK: i32 = 1000000;
// pub const LUAI_MAXSTACK: i32 = 15000;

pub const LUA_EXTRASPACE: i32 = size_of::<usize>() as i32;
pub const LUA_REGISTRYINDEX: i32 = -LUAI_MAXSTACK - 1000;
pub const LUA_IDSIZE: i32 = 60;
pub const LUA_MAXINTEGER: LUA_INTEGER = LUA_INTEGER::max_value();
pub const LUA_MININTEGER: LUA_INTEGER = LUA_INTEGER::min_value();

pub type LUA_NUMBER = f64;
pub type LUA_INTEGER = i64;
pub type LUA_UNSIGNED = u32;

pub type LUA_KCONTEXT = isize;

pub const LUA_FILEHANDLE: &str = "FILE*";
pub const LUA_VERSION_NUM: i32 = 503;

pub const LUAL_NUMSIZES: usize = size_of::<LUA_INTEGER>() * 16 + size_of::<LUA_NUMBER>();
pub const LUAL_BUFFERSIZE: u32 = 8192;

#[inline(always)]
pub unsafe fn lua_numtointeger(n: LUA_NUMBER, p: *mut LUA_INTEGER) -> c_int {
    if n >= (LUA_MININTEGER as LUA_NUMBER) && n < -(LUA_MININTEGER as LUA_NUMBER) {
        *p = n as LUA_INTEGER;
        1
    } else {
        0
    }
}
