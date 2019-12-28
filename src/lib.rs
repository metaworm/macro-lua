#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_must_use)]
#![feature(asm)]
#![feature(trace_macros)]
#![feature(fn_traits)]

use std::os::raw::{c_char, c_double, c_int, c_longlong, c_uchar, c_void};
use std::ops::Deref;

pub mod ffi;
pub mod thread;
pub mod global;

pub use ffi::{
    lua_Number, lua_Integer,
    CFunction, lua_CFunction,
    lua_Alloc, lua_Hook,
};

pub type Index = c_int;
pub type RustClosure = Box<dyn FnMut(State) -> c_int>;

mod ulua;
mod luaconf;
mod convert;
mod state;

pub use convert::*;
pub use state::*;

#[derive(Clone, Copy)]
pub struct ValRef {
    state: State,
    index: Index
}

impl ValRef {
    pub fn new(state: State, index: Index) -> Self {
        ValRef { state, index: state.abs_index(index) }
    }

    pub fn call<T: ToLuaMulti, R: FromLuaMulti>(&self, t: T) -> Result<R, CallError> {
        self.push_value(self.index);
        let top = self.get_top();
        match self.pcall({t.to_lua(&self); T::COUNT as c_int}, R::COUNT as c_int, 0) {
            ThreadStatus::Ok => R::from_lua(self, top).ok_or(CallError::ValueNotMatch),
            Status => Err(CallError::VmError(Status)),
        }
    }

    #[inline]
    pub fn is_nil(&self) -> bool { self.state.is_nil(self.index) }

    #[inline]
    pub fn is_integer(&self) -> bool { self.state.is_integer(self.index) }

    #[inline]
    pub fn to_bool(&self) -> bool { self.state.to_bool(self.index) }

    #[inline]
    pub fn check_type(&self, ty: Type) { self.state.check_type(self.index, ty); }
}

pub struct TopRef(pub ValRef);

impl Deref for TopRef {
    type Target = ValRef;
    fn deref(&self) -> &ValRef { &self.0 }
}

impl Deref for ValRef {
    type Target = State;
    fn deref(&self) -> &State { &self.state }
}

pub struct Table(pub ValRef);

impl Table {
    pub fn geti(&self, i: impl Into<lua_Integer>) -> ValRef {
        self.0.geti(self.0.index, i.into());
        self.0.val(-1)
    }

    pub fn seti<V: ToLua>(&self, i: impl Into<lua_Integer>, v: V) {
        v.to_lua(&self.0);
        self.0.seti(self.0.index, i.into());
    }

    pub fn get(&self, k: &str) -> ValRef {
        self.0.get_field(self.0.index, k);
        self.0.val(-1)
    }

    pub fn set<V: ToLua>(&self, k: &str, v: V) {
        v.to_lua(&self.0);
        self.0.set_field(self.0.index, k);
    }

    #[inline]
    pub fn getp<T>(&self, p: *const T) -> ValRef {
        self.0.raw_getp(self.0.index, p);
        self.0.val(-1)
    }

    #[inline]
    pub fn setp<T, V: ToLua>(&self, k: *const T, v: V) {
        v.to_lua(&self.0);
        self.0.raw_setp(self.0.index, k);
    }

    #[inline]
    pub fn reference<V: ToLua>(&self, v: V) -> Reference {
        v.to_lua(&self.0);
        self.0.reference(self.0.index)
    }

    #[inline]
    pub fn unreference(&self, r: Reference) {
        self.0.unreference(self.0.index, r);
    }
}

pub trait FromIndex: Sized {
    /// Converts the value on top of the stack of a Lua state to a value of type
    /// `Option<Self>`.
    fn from_lua(state: &State, index: Index) -> Self;
}

impl FromIndex for &str {
    #[inline]
    fn from_lua(state: &State, index: Index) -> &'static str {
        if let Some(s) = state.to_str(index) { s } else { state.arg_error(index, ""); }
    }
}

impl FromIndex for Option<&str> {
    #[inline]
    fn from_lua(s: &State, index: Index) -> Option<&'static str> { s.to_str(index) }
}

impl FromIndex for &[u8] {
    #[inline]
    fn from_lua(s: &State, index: Index) -> &'static [u8] {
        match s.to_bytes(index) { Some(r) => r, None => s.arg_error(index, "") }
    }
}

impl FromIndex for Option<&[u8]> {
    #[inline]
    fn from_lua(s: &State, index: Index) -> Option<&'static [u8]> { s.to_bytes(index) }
}

macro_rules! impl_number {
    (@int $($t:ty)*) => {
        $(
            impl FromIndex for $t {
                fn from_lua(s: &State, index: Index) -> $t {
                    if s.is_integer(index) { s.to_integer(index) as $t }
                    else { s.arg_error(index, "") }
                }
            }

            impl FromIndex for Option<$t> {
                fn from_lua(s: &State, index: Index) -> Option<$t> {
                    if s.is_integer(index) {
                        Some(s.to_integer(index) as $t)
                    } else { None }
                }
            }
        )*
    };

    (@float $($t:ty)*) => {
        $(
            impl FromIndex for $t {
                fn from_lua(s: &State, index: Index) -> $t {
                    if s.is_number(index) { s.to_number(index) as $t }
                    else { s.arg_error(index, "") }
                }
            }

            impl FromIndex for Option<$t> {
                fn from_lua(s: &State, index: Index) -> Option<$t> {
                    if s.is_number(index) {
                        Some(s.to_number(index) as $t)
                    } else { None }
                }
            }
        )*
    }
}

impl_number!(@int i8 u8 i16 u16 i32 u32 i64 u64 isize usize);
impl_number!(@float f32 f64);

impl FromIndex for bool {
    #[inline]
    fn from_lua(state: &State, index: Index) -> bool { state.to_bool(index) }
}

impl FromIndex for Option<bool> {
    #[inline]
    fn from_lua(s: &State, index: Index) -> Option<bool> {
        if s.is_bool(index) { Some(s.to_bool(index)) } else { None }
    }
}

impl FromIndex for ValRef {
    #[inline]
    fn from_lua(s: &State, index: Index) -> ValRef { ValRef { state: *s, index } }
}

impl FromIndex for Value {
    #[inline]
    fn from_lua(s: &State, index: Index) -> Value { s.value(index) }
}

#[macro_export]
macro_rules! cfn {
    (@unpack $s:ident $i:tt) => {};
    (@unpack $s:ident $i:tt $($v:ident : $t:ty)+) => {
        let mut i = $i;
        $(let $v: $t = FromIndex::from_lua(&$s, i); i += 1;)+
    };

    (@define_fn $name:ident $l:ident $body:block) => {
        unsafe extern "C" fn $name($l: *mut $crate::ffi::lua_State) -> i32 $body
    };

    (@define $l:ident $body:block) => {{
        cfn! { @define_fn function $l $body }
        function as CFunction
    }};

    (@body_option $s:ident $body:block) => { $body };
    (@body_option $s:ident push $body:block ) => { $s.pushx($body) };
    (@body_option $s:ident r0 $body:block) => { $body; return 0; };
    (@body_option $s:ident r1 $body:block) => { $body; return 1; };

    (($s:ident $(,$v:ident : $t:ty)*) $($body_option:ident)? $body:block) => {
        cfn!(@define l {
            let $s = $crate::State::from_ptr(l);
            cfn!(@unpack $s 1 $($v: $t)*);
            cfn!{@body_option $s $($body_option)? $body}
        })
    };

    (|$s:ident $(,$v:ident : $t:ty)*| $($body_option:ident)? $body:block) => {
        cfn! { @define l {
            let $s = $crate::State::from_ptr(l);
            cfn!(@unpack $s 1 $($v: $t)*);
            cfn!{@body_option $s $($body_option)? $body}
        }}
    };
}

#[macro_export]
macro_rules! metatable {
    (@method, $t:ty, ($s:ident, $this:ident, $($v:ident : $a:ty),*) $($body_option:ident)? $body:block) => {
        cfn!(@define l {
            let $s = $crate::State::from_ptr(l);
            let $this: &mut $t = std::mem::transmute($s.to_userdata(1));
            cfn!(@unpack $s 2 $($v: $a)*);
            cfn!(@body_option $s $($body_option)? $body)
        })
    };

    (@option) => {};
    (@option IndexSelf $meta:ident) => { $meta.set("__index", $meta.0); };

    (
        $t:tt($s:ident: State, $this:ident: Self) $($option:ident)?;
        $($name:tt($($arg_def:tt)*) $($body_option:ident)? $body:block)*
    ) => {{
        fn init_metatable(meta: $crate::Table, $s: $crate::State) {
            metatable!(@option $($option meta)?);
            $(
                meta.set($name, metatable!(
                    @method, $t, ($s, $this, $($arg_def)*)
                    $($body_option)? $body
                ));
            )*
        }
        init_metatable
    }};

    (const $name:ident = $($rest:tt)*) => { const $name: InitMetatable = metatable!($($rest)*); };
    (static $name:ident = $($rest:tt)*) => { static $name: InitMetatable = metatable!($($rest)*); };
}

#[macro_export]
macro_rules! struct_to_table {
    (@count) => { 0 };
    (@count $t:ident $(tts:ident)*) => { struct_to_table!(@count $($tts)*) + 1 };

    (@set $table:ident, $t:ident, $field:ident) => { $table.set(stringify!($field), $t.$field); };
    (@set $table:ident, $t:ident, $field:ident, $expr:expr) => { $table.set(stringify!($field), $expr); };

    ($s:ident, $t:ident; $($field:ident $(= $expr:expr)?),*) => {{
        let t = $s.table(0, 0); // TODO
        $(struct_to_table!(@set t, $t, $field $(, $expr)?);)*
    }};

    ($s:ident, |$t:ident| $($tts:tt)*) => { |$t| struct_to_table!($s, $t; $($tts)*) };
}