
use crate::*;

pub(crate) fn init_global(this: State) {
    let g = this.global();

    #[cfg(target_arch = "x86_64")]
    g.set("ARCH", "x64");
    #[cfg(target_arch = "x86")]
    g.set("ARCH", "x86");

    g.set("newuserdata", cfn!(|s, size: usize| r1 {
        s.new_userdata(size);
    }));

    g.set("topointer", cfn!(|s| r1 {
        if s.is_integer(1) {
            s.push_value(1);
        } else {
            s.push_integer(s.to_pointer(1) as lua_Integer);
        };
    }));

    g.set("getmem", cfn!((s, ptr: usize) {
        if s.is_integer(2) {
            let size = s.to_integer(2) as usize;
            s.push_bytes(std::slice::from_raw_parts(ptr as *const u8, size));
        } else if let Some(c) = s.to_bytes(2) {
            match c[0] {
                b'b' => { s.push_integer(*(ptr as *const i8) as lua_Integer); }
                b'B' => { s.push_integer(*(ptr as *const u8) as lua_Integer); }
                b's' => { s.push_integer(*(ptr as *const i16) as lua_Integer); }
                b'S' => { s.push_integer(*(ptr as *const u16) as lua_Integer); }
                b'i' => { s.push_integer(*(ptr as *const i32) as lua_Integer); }
                b'I' => { s.push_integer(*(ptr as *const u32) as lua_Integer); }
                b'l' => { s.push_integer(*(ptr as *const i64) as lua_Integer); }
                b'L' => { s.push_integer(*(ptr as *const u64) as lua_Integer); }
                b'f' => { s.push_number(*(ptr as *const f32) as lua_Number); }
                b'd' => { s.push_number(*(ptr as *const f64) as lua_Number); }
                b'p' => { s.push_integer(*(ptr as *const usize) as lua_Integer); }
                Else => panic!(""),
            }
        }
        1
    }));

    g.set("setmem", cfn!((s, ptr: usize) {
        if s.is_integer(2) {
            let size = s.to_integer(2) as usize;
            s.push_bytes(std::slice::from_raw_parts(ptr as *const u8, size));
        } else if let Some(c) = s.to_bytes(2) {
            let ival = s.to_integer(3);
            let nval = s.to_integer(3);
            match c[0] {
                b'b' => { *(ptr as *mut i8) = ival as i8; }
                b'B' => { *(ptr as *mut u8) = ival as u8; }
                b's' => { *(ptr as *mut i16) = ival as i16; }
                b'S' => { *(ptr as *mut u16) = ival as u16; }
                b'i' => { *(ptr as *mut i32) = ival as i32; }
                b'I' => { *(ptr as *mut u32) = ival as u32; }
                b'l' => { *(ptr as *mut i64) = ival as i64; }
                b'L' => { *(ptr as *mut u64) = ival as u64; }
                b'f' => { *(ptr as *mut f32) = nval as f32; }
                b'd' => { *(ptr as *mut f64) = nval as f64; }
                b'p' => { *(ptr as *mut usize) = nval as usize; }
                Else => panic!(""),
            }
        }
        0
    }));
}