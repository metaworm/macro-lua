
use crate::{
    lua_Integer as Integer,
    lua_Number as Number,
    CFunction, Index,
};
use crate::{State, ValRef, TopRef};
use crate::ffi::{self, lua_State};

use libc::c_int;
use std::mem;

/// Trait for types that can be pushed onto the stack of a Lua state.
///
/// It is important that implementors of this trait ensure that `to_lua`
/// behaves like one of the `lua_push*` functions for consistency.
pub trait ToLua {
    /// Pushes a value of type `Self` onto the stack of a Lua state.
    fn to_lua(self, state: &State);
}

impl<'a> ToLua for &'a str {
    fn to_lua(self, state: &State) {
        state.push_string(self);
    }
}

impl ToLua for String {
    fn to_lua(self, state: &State) {
        state.push_string(&self);
    }
}

impl<'a> ToLua for &'a [u8] {
    fn to_lua(self, state: &State) {
        state.push_bytes(self);
    }
}

impl ToLua for ValRef {
    fn to_lua(self, state: &State) {
        state.push_value(self.index);
    }
}

impl ToLua for TopRef {
    fn to_lua(self, state: &State) {
        let top = state.get_top();
        if top > self.index {
            state.push_value(self.index);
        } else if top < self.index {
            panic!("");
        }
    }
}

impl ToLua for Number {
    fn to_lua(self, state: &State) {
        state.push_number(self)
    }
}

impl ToLua for f32 {
    fn to_lua(self, state: &State) {
        state.push_number(self as Number)
    }
}

impl ToLua for bool {
    fn to_lua(self, state: &State) {
        state.push_bool(self)
    }
}

impl ToLua for CFunction {
    fn to_lua(self, state: &State) {
        state.push_fn(Some(self))
    }
}

impl<T: ToLua + Copy> ToLua for &T {
    fn to_lua(self, state: &State) {
        (*self).to_lua(state);
    }
}

impl<T: ToLua> ToLua for Option<T> {
    fn to_lua(self, state: &State) {
        match self {
            Some(value) => value.to_lua(state),
            None        => state.push_nil(),
        }
    }
}

impl<T: ToLua> ToLua for Vec<T> {
    fn to_lua(self, state: &State) {
        let r = state.table(self.len() as i32, 0);
        let mut i = 1;
        for e in self.into_iter() { r.seti(i, e); i += 1; }
    }
}

// impl<I: Iterator<Item=T>> ToLua for I where T: ToLua {
//     fn to_lua(self, state: &State) {
//         let r = state.table(0, 0);
//         let mut i = 1;
//         for e in self { r.seti(i, e); i += 1; }
//     }
// }

/// Trait for types that can be taken from the Lua stack.
///
/// It is important that implementors of this trait ensure that `from_lua`
/// behaves like one of the `lua_to*` functions for consistency.
pub trait FromLua: Sized {
    /// Converts the value on top of the stack of a Lua state to a value of type
    /// `Option<Self>`.
    fn from_lua(state: &State, index: Index) -> Option<Self>;
}

impl FromLua for String {
    fn from_lua(state: &State, index: Index) -> Option<String> {
        state.to_str(index).map(ToOwned::to_owned)
    }
}

impl FromLua for &str {
    fn from_lua(state: &State, index: Index) -> Option<&'static str> {
        state.to_str(index)
    }
}

impl<'a> FromLua for &'a [u8] {
    fn from_lua(state: &State, index: Index) -> Option<&'a [u8]> {
        let mut len = 0;
        let ptr = state.tolstring(index, &mut len);
        if ptr.is_null() { None } else {
            Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) })
        }
    }
}

impl FromLua for Number {
    fn from_lua(state: &State, index: Index) -> Option<Number> {
        if state.is_number(index) {
            Some(state.to_number(index))
        } else {
            None
        }
    }
}

impl FromLua for bool {
    fn from_lua(state: &State, index: Index) -> Option<bool> {
        Some(state.to_bool(index))
    }
}

pub struct StrictBool(pub bool);

impl FromLua for StrictBool {
    fn from_lua(state: &State, index: Index) -> Option<StrictBool> {
        if state.is_bool(index) {
            Some(StrictBool(state.to_bool(index)))
        } else { None }
    }
}

impl<T: FromLua> FromLua for Option<T> {
    fn from_lua(state: &State, index: Index) -> Option<Option<T>> {
        Some(T::from_lua(state, index))
    }
}

macro_rules! impl_integer {
    ($($t:ty) *) => {
        $(
        impl ToLua for $t {
            #[inline(always)]
            fn to_lua(self, state: &State) {
                state.push_integer(self as Integer);
            }
        }

        impl FromLua for $t {
            #[inline(always)]
            fn from_lua(state: &State, index: Index) -> Option<$t> {
                if state.is_integer(index) {
                    Some(state.to_integer(index) as $t)
                } else {
                    None
                }
            }
        }
        )*
    }
}

impl_integer!(isize usize u8 u16 u32 u64 i8 i16 i32 Integer);

pub trait ToLuaMulti: Sized {
    const COUNT: usize = 0;
    fn to_lua(self, state: &State) {}
}

pub trait FromLuaMulti: Sized {
    const COUNT: usize = 0;
    fn from_lua(state: &State, begin: Index) -> Option<Self> { None }
}

impl ToLuaMulti for () {}

impl FromLuaMulti for () {
    const COUNT: usize = 0;
    fn from_lua(state: &State, begin: Index) -> Option<Self> { Some(()) }
}

impl<T: ToLua> ToLuaMulti for T {
    const COUNT: usize = 1;
    fn to_lua(self, state: &State) {
        ToLua::to_lua(self, state);
    }
}

impl<T: FromLua> FromLuaMulti for T {
    const COUNT: usize = 1;
    fn from_lua(state: &State, begin: Index) -> Option<Self> {
        T::from_lua(state, begin)
    }
}

pub trait PushClosure<FN, ARGS, RET> {
    fn push_closure(&self, f: FN) -> TopRef;
}

pub trait PushMethod<T, FN, RET> {
    fn push_method(&self, mehtod: FN) -> TopRef;
}

use std::marker::PhantomData;
pub(crate) struct Method<T> {
    t1: PhantomData<T>,
}

impl<T: Sized> Method<T> where {
    pub unsafe extern "C" fn lua_fn(l: *mut lua_State) -> c_int {
        let state = State::from_ptr(l);
        let p: &T = mem::transmute(state.to_userdata(1));
        let fp = state.to_pointer(ffi::lua_upvalueindex(1));
        let fp: fn(&T, State) -> c_int = mem::transmute(fp);
        fp(p, state)
    }
}

macro_rules! replace_expr {
    ($_t:tt $sub:expr) => {$sub};
}

macro_rules! count_tts {
    ($($tts:tt)*) => {0usize $(+ replace_expr!($tts 1usize))*};
}

macro_rules! impl_tuple {
    ($(($x:ident, $i:tt)) +) => (
        impl<$($x,)*> ToLuaMulti for ($($x,)*) where $($x: ToLua + Copy,)* {
            const COUNT: usize = (count_tts!($($x)*));

            #[inline(always)]
            fn to_lua(self, state: &State) {
                $(state.push(self.$i);)*
            }
        }

        impl<$($x,)*> FromLuaMulti for ($($x,)*) where $($x: FromLua,)* {
            const COUNT: usize = (count_tts!($($x)*));

            #[inline(always)]
            fn from_lua(state: &State, begin: Index) -> Option<Self> {
                Some((
                    $($x::from_lua(state, begin + $i)?,)*
                ))
            }
        }

        impl<FN, RET $(,$x: FromLua)*> PushClosure<FN, ($($x,)*), RET> for State
        where FN: Fn($($x,)*) -> RET + 'static, RET: ToLuaMulti {
            fn push_closure(&self, closure: FN) -> TopRef {
                self.rust_closure(move |state| {
                    std::ops::Fn::call(
                        &closure,
                        state.args::<($($x,)*)>(1)
                    ).to_lua(&state);
                    RET::COUNT as c_int
                })
            }
        }

        // impl<T, RET, $($x,)*> Method<T, ($($x,)*), RET> where $($x: FromLua,)* RET: ToLuaMulti {
        //     pub unsafe extern "C" fn lua_fn(l: *mut lua_State) -> c_int {
        //         let state = State::from_ptr(l);
        //         let p: &T = mem::transmute(state.to_userdata(1));
        //         let fp = state.to_pointer(ffi::lua_upvalueindex(1));
        //         let fp: fn(&T, $($x,)*) -> RET = mem::transmute(fp);

        //         fp(p
        //             $(,$x::from_lua(&state, 2 + $i).unwrap())*
        //         );

        //         RET::COUNT as c_int
        //     }

        //     #[inline]
        //     pub fn to_lua(state: &State, fp: fn(&T $(,$x)*) -> RET) {
        //         state.push_light_userdata(fp as usize as *mut usize);
        //         state.push_cclosure(Some(Self::lua_fn), 1);
        //     }
        // }
    );
}

impl_tuple!((A,0));
impl_tuple!((A,0) (B,1));
impl_tuple!((A,0) (B,1) (C,2));
impl_tuple!((A,0) (B,1) (C,2) (D,3));
impl_tuple!((A,0) (B,1) (C,2) (D,3) (E,4));
impl_tuple!((A,0) (B,1) (C,2) (D,3) (E,4) (F,5));
impl_tuple!((A,0) (B,1) (C,2) (D,3) (E,4) (F,5) (G,6));
impl_tuple!((A,0) (B,1) (C,2) (D,3) (E,4) (F,5) (G,6) (H,7));
impl_tuple!((A,0) (B,1) (C,2) (D,3) (E,4) (F,5) (G,6) (H,7) (I,8));

pub trait UserData {
    fn __index(&self, state: State) -> c_int {
        0
    }

    fn __newindex(&self, state: State) -> c_int {
        0
    }
}