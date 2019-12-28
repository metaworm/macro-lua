
use crate::*;
use crate::ffi::*;
use crate::convert::{ToLua, FromLua, ToLuaMulti, FromLuaMulti, Method};

use std::{mem, ptr, str, slice, any};
use std::mem::MaybeUninit;
use std::ffi::{CString, CStr};
use std::ops::DerefMut;
use std::sync::Mutex;

use libc::{c_int, c_void, c_char, size_t};
use bitflags::*;

pub type InitMetatable = fn(Table, State);

/// Arithmetic operations for `lua_arith`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arithmetic {
    Add = LUA_OPADD as isize,
    Sub = LUA_OPSUB as isize,
    Mul = LUA_OPMUL as isize,
    Mod = LUA_OPMOD as isize,
    Pow = LUA_OPPOW as isize,
    Div = LUA_OPDIV as isize,
    IDiv = LUA_OPIDIV as isize,
    BAnd = LUA_OPBAND as isize,
    BOr = LUA_OPBOR as isize,
    BXor = LUA_OPBXOR as isize,
    Shl = LUA_OPSHL as isize,
    Shr = LUA_OPSHR as isize,
    Unm = LUA_OPUNM as isize,
    BNot = LUA_OPBNOT as isize,
}

/// Comparison operations for `lua_compare`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Eq = LUA_OPEQ as isize,
    Lt = LUA_OPLT as isize,
    Le = LUA_OPLE as isize,
}

/// Status of a Lua state.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadStatus {
    Ok = LUA_OK as isize,
    Yield = LUA_YIELD as isize,
    RuntimeError = LUA_ERRRUN as isize,
    SyntaxError = LUA_ERRSYNTAX as isize,
    MemoryError = LUA_ERRMEM as isize,
    GcError = LUA_ERRGCMM as isize,
    MessageHandlerError = LUA_ERRERR as isize,
    FileError = LUA_ERRFILE as isize,
}

pub enum CallError {
    ValueNotMatch,
    VmError(ThreadStatus),
}

impl ThreadStatus {
    fn from_c_int(i: c_int) -> ThreadStatus {
        match i {
            LUA_OK => ThreadStatus::Ok,
            LUA_YIELD => ThreadStatus::Yield,
            LUA_ERRRUN => ThreadStatus::RuntimeError,
            LUA_ERRSYNTAX => ThreadStatus::SyntaxError,
            LUA_ERRMEM => ThreadStatus::MemoryError,
            LUA_ERRGCMM => ThreadStatus::GcError,
            LUA_ERRERR => ThreadStatus::MessageHandlerError,
            LUA_ERRFILE => ThreadStatus::FileError,
            _ => panic!("Unknown Lua error code: {}", i),
        }
    }

    /// Returns `true` for error statuses and `false` for `Ok` and `Yield`.
    pub fn is_err(self) -> bool {
        match self {
            ThreadStatus::RuntimeError |
                ThreadStatus::SyntaxError |
                ThreadStatus::MemoryError |
                ThreadStatus::GcError |
                ThreadStatus::MessageHandlerError |
                ThreadStatus::FileError => true,
            ThreadStatus::Ok |
                ThreadStatus::Yield => false,
        }
    }
}

/// Options for the Lua garbage collector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GcOption {
    Stop = LUA_GCSTOP as isize,
    Restart = LUA_GCRESTART as isize,
    Collect = LUA_GCCOLLECT as isize,
    Count = LUA_GCCOUNT as isize,
    CountBytes = LUA_GCCOUNTB as isize,
    Step = LUA_GCSTEP as isize,
    SetPause = LUA_GCSETPAUSE as isize,
    SetStepMul = LUA_GCSETSTEPMUL as isize,
    IsRunning = LUA_GCISRUNNING as isize,
}

/// Represents all possible Lua data types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    None = LUA_TNONE as isize,
    Nil = LUA_TNIL as isize,
    Boolean = LUA_TBOOLEAN as isize,
    LightUserdata = LUA_TLIGHTUSERDATA as isize,
    Number = LUA_TNUMBER as isize,
    String = LUA_TSTRING as isize,
    Table = LUA_TTABLE as isize,
    Function = LUA_TFUNCTION as isize,
    Userdata = LUA_TUSERDATA as isize,
    Thread = LUA_TTHREAD as isize,
    Invalid,
}

impl Type {
    fn from_c_int(i: c_int) -> Type {
        match i {
            LUA_TNIL => Type::Nil,
            LUA_TBOOLEAN => Type::Boolean,
            LUA_TLIGHTUSERDATA => Type::LightUserdata,
            LUA_TNUMBER => Type::Number,
            LUA_TSTRING => Type::String,
            LUA_TTABLE => Type::Table,
            LUA_TFUNCTION => Type::Function,
            LUA_TUSERDATA => Type::Userdata,
            LUA_TTHREAD => Type::Thread,
            _ => Type::Invalid,
        }
    }
}

pub enum Value {
    None,
    Nil,
    Int(LUA_INTEGER),
    Num(LUA_NUMBER),
    Str(&'static str),
    Bool(bool),
    LightUserdata,
    Table,
    Function,
    Userdata,
    Thread,
}

/// Represents all built-in libraries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Library {
    Base,
    Coroutine,
    Table,
    Io,
    Os,
    String,
    Utf8,
    Bit32,
    Math,
    Debug,
    Package,
}

impl Library {
    /// The name of the module in lua code
    pub fn name(&self) -> &'static str {
        use self::Library::*;
        match *self {
            Base => "_G",
            Coroutine => "coroutine",
            Table => "table",
            Io => "io",
            Os => "os",
            String => "string",
            Utf8 => "utf8",
            Bit32 => "bit32",
            Math => "math",
            Debug => "debug",
            Package => "package",
        }
    }
    /// Returns C function that may be used to load the library
    pub fn loader(&self) -> unsafe extern fn (L: *mut lua_State) -> c_int {
        use self::Library::*;
        match *self {
            Base => luaopen_base,
            Coroutine => luaopen_coroutine,
            Table => luaopen_table,
            Io => luaopen_io,
            Os => luaopen_os,
            String => luaopen_string,
            Utf8 => luaopen_utf8,
            Bit32 => luaopen_bit32,
            Math => luaopen_math,
            Debug => luaopen_debug,
            Package => luaopen_package,
        }
    }
}

/// Type of Lua references generated through `reference` and `unreference`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reference(c_int);

/// The result of `reference` for `nil` values.
pub const REFNIL: Reference = Reference(LUA_REFNIL);

/// A value that will never be returned by `reference`.
pub const NOREF: Reference = Reference(LUA_REFNIL);

impl Reference {
    /// Returns `true` if this reference is equal to `REFNIL`.
    pub fn is_nil_ref(self) -> bool {
        self == REFNIL
    }

    /// Returns `true` if this reference is equal to `NOREF`.
    pub fn is_no_ref(self) -> bool {
        self == NOREF
    }

    /// Convenience function that returns the value of this reference.
    pub fn value(self) -> c_int {
        let Reference(value) = self;
        value
    }
}

bitflags! {
    #[doc="Hook point masks for `lua_sethook`."]
    flags HookMask: c_int {
        #[doc="Called when the interpreter calls a function."]
        const MASKCALL  = LUA_MASKCALL,
        #[doc="Called when the interpreter returns from a function."]
        const MASKRET   = LUA_MASKRET,
        #[doc="Called when the interpreter is about to start the execution of a new line of code."]
        const MASKLINE  = LUA_MASKLINE,
        #[doc="Called after the interpreter executes every `count` instructions."]
        const MASKCOUNT = LUA_MASKCOUNT
    }
}

unsafe extern fn continue_func<F>(st: *mut lua_State, status: c_int, ctx: lua_KContext) -> c_int
where F: FnOnce(&mut State, ThreadStatus) -> c_int
{
    mem::transmute::<_, Box<F>>(ctx)(&mut State::from_ptr(st), ThreadStatus::from_c_int(status))
}

/// Box for extra data.
pub type Extra = Box<dyn any::Any + 'static + Send>;
type ExtraHolder = *mut *mut Mutex<Option<Extra>>;

unsafe extern fn alloc_func(_: *mut c_void, ptr: *mut c_void, old_size: size_t, new_size: size_t) -> *mut c_void {
    // In GCC and MSVC, malloc uses an alignment calculated roughly by:
    //   max(2 * sizeof(size_t), alignof(long double))
    // The stable high-level API used here does not expose alignment directly, so
    // we get as close as possible by using usize to determine alignment. Lua
    // seems unlikely to require 16-byte alignment for any of its purposes.

    #[inline]
    fn divide_size(size: size_t) -> usize {
        1 + (size - 1) / mem::size_of::<usize>()
    }

    let ptr = ptr as *mut usize;
    if new_size == 0 {
        // if new_size is 0, act like free()
        if !ptr.is_null() {
            // Lua promises to provide the correct old_size
            drop(Vec::<usize>::from_raw_parts(ptr, 0, divide_size(old_size)));
        }
        ptr::null_mut()
    } else {
        // otherwise, act like realloc()
        let mut vec;
        if ptr.is_null() {
            // old_size is a type indicator, not used here
            vec = Vec::<usize>::with_capacity(divide_size(new_size));
        } else {
            // Lua promises to provide the correct old_size
            if new_size > old_size {
                // resulting capacity should be new_size
                vec = Vec::<usize>::from_raw_parts(ptr, 0, divide_size(old_size));
                vec.reserve_exact(divide_size(new_size));
            } else {
                // Lua assumes this will never fail
                vec = Vec::<usize>::from_raw_parts(ptr, divide_size(new_size), divide_size(old_size));
                vec.shrink_to_fit();
            }
        }
        let res = vec.as_mut_ptr();
        mem::forget(vec); // don't deallocate
        res as *mut c_void
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct State(*mut lua_State);

unsafe impl Send for State {}

impl State {
    /// Initializes a new Lua state. This function does not open any libraries
    /// by default. Calls `lua_newstate` internally.
    pub fn new() -> State {
        unsafe { State(luaL_newstate()) }
    }

    /// Constructs a wrapper `State` from a raw pointer. This is suitable for use
    /// inside of native functions that accept a `lua_State` to obtain a wrapper.
    #[inline(always)]
    pub unsafe fn from_ptr(L: *mut lua_State) -> State { State(L) }

    /// Returns an unsafe pointer to the wrapped `lua_State`.
    pub fn as_ptr(&self) -> *mut lua_State { self.0 }

    /// Maps to `luaL_openlibs`.
    pub fn open_libs(&self) {
        unsafe { luaL_openlibs(self.0) }
        // Init ulua
        use crate::{thread, global};

        self.load_global();
        self.open_thread();
    }

    #[inline]
    pub fn load_global(&self) { self.balance_with(global::init_global); }

    #[inline]
    pub fn open_thread(&self) { self.balance_with(thread::init_thread); }

    /// Preloads library, i.e. it's not exposed, but can be required
    pub fn preload_library(&self, lib: Library) {
        unsafe {
            let pre = CString::new("_PRELOAD").unwrap();
            luaL_getsubtable(self.0, LUA_REGISTRYINDEX, pre.as_ptr());
            self.push_fn(Some(lib.loader()));
            self.set_field(-2, lib.name());
            self.pop(1);  /* remove lib */
        }
    }

    /// Loads a built-in library and exposes it into lua code
    pub fn load_library(&self, lib: Library) {
        self.requiref(lib.name(), Some(lib.loader()), true);
        self.pop(1);  /* remove lib */
    }

    /// Maps to `luaopen_base`.
    #[inline]
    pub fn open_base(&self) -> c_int {
        unsafe { luaopen_base(self.0) }
    }

    /// Maps to `luaopen_coroutine`.
    #[inline]
    pub fn open_coroutine(&self) -> c_int {
        unsafe { luaopen_coroutine(self.0) }
    }

    /// Maps to `luaopen_table`.
    #[inline]
    pub fn open_table(&mut self) -> c_int {
        unsafe { luaopen_table(self.0) }
    }

    /// Maps to `luaopen_io`.
    #[inline]
    pub fn open_io(&self) -> c_int {
        unsafe { luaopen_io(self.0) }
    }

    /// Maps to `luaopen_os`.
    #[inline]
    pub fn open_os(&self) -> c_int {
        unsafe { luaopen_os(self.0) }
    }

    /// Maps to `luaopen_string`.
    #[inline]
    pub fn open_string(&mut self) -> c_int {
        unsafe { luaopen_string(self.0) }
    }

    /// Maps to `luaopen_utf8`.
    #[inline]
    pub fn open_utf8(&self) -> c_int {
        unsafe { luaopen_utf8(self.0) }
    }

    /// Maps to `luaopen_math`.
    #[inline]
    pub fn open_math(&self) -> c_int {
        unsafe { luaopen_math(self.0) }
    }

    /// Maps to `luaopen_debug`.
    #[inline]
    pub fn open_debug(&self) -> c_int {
        unsafe { luaopen_debug(self.0) }
    }

    /// Maps to `luaopen_package`.
    #[inline]
    pub fn open_package(&self) -> c_int {
        unsafe { luaopen_package(self.0) }
    }

    /// Maps to `luaL_dofile`.
    pub fn do_file(&self, filename: &str) -> ThreadStatus {
        let c_str = CString::new(filename).unwrap();
        let result = unsafe {
            luaL_dofile(self.0, c_str.as_ptr())
        };
        ThreadStatus::from_c_int(result)
    }

    /// Maps to `luaL_dostring`.
    pub fn do_string(&self, s: &str) -> ThreadStatus {
        let c_str = CString::new(s).unwrap();
        let result = unsafe {
            luaL_dostring(self.0, c_str.as_ptr())
        };
        ThreadStatus::from_c_int(result)
    }

    /// Pushes the given value onto the stack.
    #[inline(always)]
    pub fn push<T: ToLua>(&self, value: T) {
        value.to_lua(self);
    }

    //===========================================================================
    // State manipulation
    //===========================================================================
    /// Maps to `lua_close`.
    #[inline]
    pub fn close(self) {
        unsafe { lua_close(self.0); }
    }

    /// [-0, +1, m] Maps to `lua_newthread`.
    #[inline]
    pub fn new_thread(&self) -> State {
        unsafe {
            State::from_ptr(lua_newthread(self.0))
        }
    }

    /// Maps to `lua_atpanic`.
    #[inline]
    pub fn at_panic(&self, panicf: lua_CFunction) -> lua_CFunction {
        unsafe { lua_atpanic(self.0, panicf) }
    }

    /// Maps to `lua_version`.
    pub fn version(state: Option<&mut State>) -> lua_Number {
        let ptr = match state {
            Some(state) => state.0,
            None        => ptr::null_mut()
        };
        unsafe { *lua_version(ptr) }
    }

    //===========================================================================
    // Basic stack manipulation
    //===========================================================================
    /// Maps to `lua_absindex`.
    #[inline]
    pub fn abs_index(&self, idx: Index) -> Index {
        unsafe { lua_absindex(self.0, idx) }
    }

    /// Maps to `lua_gettop`.
    #[inline]
    pub fn get_top(&self) -> Index {
        unsafe { lua_gettop(self.0) }
    }

    /// Maps to `lua_settop`.
    #[inline]
    pub fn set_top(&self, index: Index) {
        unsafe { lua_settop(self.0, index) }
    }

    /// Maps to `lua_pushvalue`.
    #[inline]
    pub fn push_value(&self, index: Index) {
        unsafe { lua_pushvalue(self.0, index) }
    }

    /// Maps to `lua_rotate`.
    #[inline]
    pub fn rotate(&mut self, idx: Index, n: c_int) {
        unsafe { lua_rotate(self.0, idx, n) }
    }

    /// Maps to `lua_copy`.
    #[inline]
    pub fn copy(&mut self, from_idx: Index, to_idx: Index) {
        unsafe { lua_copy(self.0, from_idx, to_idx) }
    }

    /// Maps to `lua_checkstack`.
    #[inline]
    pub fn check_stack(&mut self, extra: c_int) -> bool {
        let result = unsafe { lua_checkstack(self.0, extra) };
        result != 0
    }

    /// Maps to `lua_xmove`.
    #[inline]
    pub fn xmove(&self, to: State, n: c_int) {
        unsafe { lua_xmove(self.0, to.0, n) }
    }

    //===========================================================================
    // Access functions (stack -> C)
    //===========================================================================
    /// Maps to `lua_isnumber`.
    #[inline]
    pub fn is_number(&self, index: Index) -> bool {
        unsafe { lua_isnumber(self.0, index) == 1 }
    }

    /// Maps to `lua_isstring`.
    #[inline]
    pub fn is_string(&self, index: Index) -> bool {
        unsafe { lua_isstring(self.0, index) == 1 }
    }

    /// Maps to `lua_iscfunction`.
    #[inline]
    pub fn is_native_fn(&self, index: Index) -> bool {
        unsafe { lua_iscfunction(self.0, index) == 1 }
    }

    /// Maps to `lua_isinteger`.
    #[inline]
    pub fn is_integer(&self, index: Index) -> bool {
        unsafe { lua_isinteger(self.0, index) == 1 }
    }

    /// Maps to `lua_isuserdata`.
    #[inline]
    pub fn is_userdata(&self, index: Index) -> bool {
        unsafe { lua_isuserdata(self.0, index) == 1 }
    }

    /// Maps to `lua_type`.
    #[inline]
    pub fn type_of(&self, index: Index) -> Type {
        let result = unsafe { lua_type(self.0, index) };
        Type::from_c_int(result)
    }

    /// Maps to `lua_typename`.
    pub fn typename_of(&self, tp: Type) -> &'static str {
        unsafe {
            let ptr = lua_typename(self.0, tp as c_int);
            let slice = CStr::from_ptr(ptr).to_bytes();
            str::from_utf8(slice).unwrap()
        }
    }

    /// Maps to `lua_tonumberx`.
    pub fn to_numberx(&self, index: Index) -> Option<lua_Number> {
        let mut isnum: c_int = 0;
        let result = unsafe { lua_tonumberx(self.0, index, &mut isnum) };
        if isnum == 0 {
            None
        } else {
            Some(result)
        }
    }

    /// Maps to `lua_tointegerx`.
    pub fn to_integerx(&self, index: Index) -> Option<lua_Integer> {
        let mut isnum: c_int = 0;
        let result = unsafe { lua_tointegerx(self.0, index, &mut isnum) };
        if isnum == 0 {
            None
        } else {
            Some(result)
        }
    }

    /// Maps to `lua_toboolean`.
    #[inline]
    pub fn to_bool(&self, index: Index) -> bool {
        let result = unsafe { lua_toboolean(self.0, index) };
        result != 0
    }

    // omitted: lua_tolstring

    /// Maps to `lua_rawlen`.
    #[inline]
    pub fn raw_len(&self, index: Index) -> size_t {
        unsafe { lua_rawlen(self.0, index) }
    }

    /// Maps to `lua_tocfunction`.
    #[inline]
    pub fn to_native_fn(&self, index: Index) -> lua_CFunction {
        let result = unsafe { lua_tocfunction(self.0, index) };
        result
    }

    /// Maps to `lua_touserdata`.
    #[inline]
    pub fn to_userdata(&self, index: Index) -> *mut c_void {
        unsafe { lua_touserdata(self.0, index) }
    }

    /// Convenience function that calls `to_userdata` and performs a cast.
    //#[unstable(reason="this is an experimental function")]
    pub unsafe fn to_userdata_typed<'a, T>(&'a mut self, index: Index) -> Option<&'a mut T> {
        mem::transmute(self.to_userdata(index))
    }

    /// Maps to `lua_tothread`.
    #[inline]
    pub fn to_thread(&self, index: Index) -> Option<State> {
        let state = unsafe { lua_tothread(self.0, index) };
        if state.is_null() {
            None
        } else {
            Some(unsafe { State::from_ptr(state) })
        }
    }

    /// Maps to `lua_topointer`.
    #[inline]
    pub fn to_pointer(&self, index: Index) -> *const c_void {
        unsafe { lua_topointer(self.0, index) }
    }

    //===========================================================================
    // Comparison and arithmetic functions
    //===========================================================================
    /// Maps to `lua_arith`.
    #[inline]
    pub fn arith(&self, op: Arithmetic) {
        unsafe { lua_arith(self.0, op as c_int) }
    }

    /// Maps to `lua_rawequal`.
    #[inline]
    pub fn raw_equal(&self, idx1: Index, idx2: Index) -> bool {
        let result = unsafe { lua_rawequal(self.0, idx1, idx2) };
        result != 0
    }

    /// Maps to `lua_compare`.
    #[inline]
    pub fn compare(&self, idx1: Index, idx2: Index, op: Comparison) -> bool {
        let result = unsafe { lua_compare(self.0, idx1, idx2, op as c_int) };
        result != 0
    }

    //===========================================================================
    // Push functions (C -> stack)
    //===========================================================================
    /// Maps to `lua_pushnil`.
    #[inline]
    pub fn push_nil(&self) {
        unsafe { lua_pushnil(self.0) }
    }

    /// Maps to `lua_pushnumber`.
    #[inline]
    pub fn push_number(&self, n: lua_Number) {
        unsafe { lua_pushnumber(self.0, n) }
    }

    /// Maps to `lua_pushinteger`.
    #[inline]
    pub fn push_integer(&self, i: lua_Integer) {
        unsafe { lua_pushinteger(self.0, i) }
    }

    // omitted: lua_pushstring

    /// Maps to `lua_pushlstring`.
    #[inline]
    pub fn push_string(&self, s: &str) {
        unsafe { lua_pushlstring(self.0, s.as_ptr() as *const _, s.len() as size_t) };
    }

    /// Maps to `lua_pushlstring`.
    #[inline]
    pub fn push_bytes(&self, s: &[u8]) {
        unsafe { lua_pushlstring(self.0, s.as_ptr() as *const _, s.len() as size_t) };
    }

    // omitted: lua_pushvfstring
    // omitted: lua_pushfstring

    /// Maps to `lua_pushcclosure`.
    #[inline]
    pub fn push_cclosure(&self, f: lua_CFunction, n: c_int) {
        unsafe { lua_pushcclosure(self.0, f, n) }
    }

    /// Maps to `lua_pushboolean`.
    #[inline]
    pub fn push_bool(&self, b: bool) {
        unsafe { lua_pushboolean(self.0, b as c_int) }
    }

    /// Maps to `lua_pushlightuserdata`. The Lua state will receive a pointer to
    /// the given value. The caller is responsible for cleaning up the data. Any
    /// code that manipulates the userdata is free to modify its contents, so
    /// memory safety is not guaranteed.
    #[inline]
    pub fn push_light_userdata<T>(&self, ud: *mut T) {
        unsafe { lua_pushlightuserdata(self.0, mem::transmute(ud)) }
    }

    /// Maps to `lua_pushthread`.
    pub fn push_thread(&self) -> bool {
        let result = unsafe { lua_pushthread(self.0) };
        result != 1
    }

    //===========================================================================
    // Get functions (Lua -> stack)
    //===========================================================================
    /// Maps to `lua_getglobal`.
    pub fn get_global(&mut self, name: &str) -> Type {
        let c_str = CString::new(name).unwrap();
        let ty = unsafe {
            lua_getglobal(self.0, c_str.as_ptr())
        };
        Type::from_c_int(ty)
    }

    /// Maps to `lua_gettable`.
    pub fn get_table(&mut self, index: Index) -> Type {
        let ty = unsafe { lua_gettable(self.0, index) };
        Type::from_c_int(ty)
    }

    /// Maps to `lua_getfield`.
    pub fn get_field(&self, index: Index, k: &str) -> Type {
        let c_str = CString::new(k).unwrap();
        let ty = unsafe {
            lua_getfield(self.0, index, c_str.as_ptr())
        };
        Type::from_c_int(ty)
    }

    /// Maps to `lua_geti`.
    pub fn geti(&self, index: Index, i: lua_Integer) -> Type {
        let ty = unsafe { lua_geti(self.0, index, i) };
        Type::from_c_int(ty)
    }

    /// [-1, +1, -] `lua_rawget`.
    pub fn raw_get(&self, index: Index) -> Type {
        let ty = unsafe { lua_rawget(self.0, index) };
        Type::from_c_int(ty)
    }

    /// Maps to `lua_rawgeti`.
    pub fn raw_geti(&self, index: Index, n: lua_Integer) -> Type {
        let ty = unsafe { lua_rawgeti(self.0, index, n) };
        Type::from_c_int(ty)
    }

    /// Maps to `lua_rawgetp`.
    #[inline]
    pub fn raw_getp<T>(&self, index: Index, p: *const T) -> Type {
        let ty = unsafe { lua_rawgetp(self.0, index, mem::transmute(p)) };
        Type::from_c_int(ty)
    }

    /// Maps to `lua_createtable`.
    #[inline]
    pub fn create_table(&self, narr: c_int, nrec: c_int) {
        unsafe { lua_createtable(self.0, narr, nrec) }
    }

    /// Maps to `lua_newuserdata`. The pointer returned is owned by the Lua state
    /// and it will be garbage collected when it is no longer in use or the state
    /// is closed. To specify custom cleanup behavior, use a `__gc` metamethod.
    pub fn new_userdata(&self, sz: size_t) -> *mut c_void {
        unsafe { lua_newuserdata(self.0, sz) }
    }

    /// Convenience function that uses type information to call `new_userdata`
    /// and perform a cast.
    ///
    /// # Example
    ///
    /// ```ignore
    /// unsafe { *state.new_userdata_typed() = MyStruct::new(...); }
    /// state.set_metatable_from_registry("MyStruct");
    /// ```
    //#[unstable(reason="this is an experimental function")]
    pub fn new_userdata_typed<T>(&mut self) -> *mut T {
        self.new_userdata(mem::size_of::<T>() as size_t) as *mut T
    }

    /// Maps to `lua_getmetatable`.
    pub fn get_metatable(&mut self, objindex: Index) -> bool {
        let result = unsafe { lua_getmetatable(self.0, objindex) };
        result != 0
    }

    /// Maps to `lua_getuservalue`.
    pub fn get_uservalue(&self, idx: Index) -> Type {
        let result = unsafe { lua_getuservalue(self.0, idx) };
        Type::from_c_int(result)
    }

    //===========================================================================
    // Set functions (stack -> Lua)
    //===========================================================================
    /// Maps to `lua_setglobal`.
    pub fn set_global(&self, var: &str) {
        let c_str = CString::new(var).unwrap();
        unsafe { lua_setglobal(self.0, c_str.as_ptr()) }
    }

    /// Maps to `lua_settable`.
    pub fn set_table(&self, idx: Index) {
        unsafe { lua_settable(self.0, idx) }
    }

    /// Maps to `lua_setfield`.
    pub fn set_field(&self, idx: Index, k: &str) {
        let c_str = CString::new(k).unwrap();
        unsafe { lua_setfield(self.0, idx, c_str.as_ptr()) }
    }

    /// Maps to `lua_seti`.
    pub fn seti(&self, idx: Index, n: lua_Integer) {
        unsafe { lua_seti(self.0, idx, n) }
    }

    /// Maps to `lua_rawset`.
    pub fn raw_set(&self, idx: Index) {
        unsafe { lua_rawset(self.0, idx) }
    }

    /// Maps to `lua_rawseti`.
    pub fn raw_seti(&self, idx: Index, n: lua_Integer) {
        unsafe { lua_rawseti(self.0, idx, n) }
    }

    /// Maps to `lua_rawsetp`.
    #[inline]
    pub fn raw_setp<T>(&self, idx: Index, p: *const T) {
        unsafe { lua_rawsetp(self.0, idx, mem::transmute(p)) }
    }

    /// Maps to `lua_setmetatable`.
    pub fn set_metatable(&self, objindex: Index) {
        unsafe { lua_setmetatable(self.0, objindex) };
    }

    /// Maps to `lua_setuservalue`.
    pub fn set_uservalue(&self, idx: Index) {
        unsafe { lua_setuservalue(self.0, idx) }
    }

    //===========================================================================
    // 'load' and 'call' functions (load and run Lua code)
    //===========================================================================
    /// Maps to `lua_callk`.
    // pub fn callk<F>(&mut self, nargs: c_int, nresults: c_int, continuation: F)
    //     where F: FnOnce(&mut State, ThreadStatus) -> c_int
    //     {
    //         let func = continue_func::<F>;
    //         unsafe {
    //             let ctx = mem::transmute(Box::new(continuation));
    //             lua_callk(self.0, nargs, nresults, ctx, Some(func));
    //             // no yield occurred, so call the continuation
    //             func(self.0, LUA_OK, ctx);
    //         }
    //     }

    /// Maps to `lua_call`.
    // #[inline]
    // pub fn call(&self, nargs: c_int, nresults: c_int) {
    //     unsafe { lua_call(self.0, nargs, nresults) }
    // }

    /// Maps to `lua_pcallk`.
    // pub fn pcallk<F>(&mut self, nargs: c_int, nresults: c_int, msgh: c_int, continuation: F) -> c_int
    //     where F: FnOnce(&mut State, ThreadStatus) -> c_int
    //     {
    //         let func = continue_func::<F>;
    //         unsafe {
    //             let ctx = mem::transmute(Box::new(continuation));
    //             // lua_pcallk only returns if no yield occurs, so call the continuation
    //             func(self.0, lua_pcallk(self.0, nargs, nresults, msgh, ctx, Some(func)), ctx)
    //         }
    //     }

    /// Maps to `lua_pcall`.
    #[inline]
    pub fn pcall(&self, nargs: c_int, nresults: c_int, msgh: c_int) -> ThreadStatus {
        let result = unsafe {
            lua_pcall(self.0, nargs, nresults, msgh)
        };
        ThreadStatus::from_c_int(result)
    }

    // TODO: mode typing?
    /// Maps to `lua_load`.
    // pub fn load<'l, F>(&'l mut self, mut reader: F, source: &str, mode: &str) -> ThreadStatus
    //     where F: FnMut(&mut State) -> &'l [u8]
    //     {
    //         unsafe extern fn read<'l, F>(st: *mut lua_State, ud: *mut c_void, sz: *mut size_t) -> *const c_char
    //             where F: FnMut(&mut State) -> &'l [u8]
    //             {
    //                 let mut state = State::from_ptr(st);
    //                 let slice = mem::transmute::<_, &mut F>(ud)(&mut state);
    //                 *sz = slice.len() as size_t;
    //                 slice.as_ptr() as *const _
    //             }
    //         let source_c_str = CString::new(source).unwrap();
    //         let mode_c_str = CString::new(mode).unwrap();
    //         let result = unsafe {
    //             lua_load(self.0, Some(read::<F>), mem::transmute(&mut reader), source_c_str.as_ptr(), mode_c_str.as_ptr())
    //         };
    //         ThreadStatus::from_c_int(result)
    //     }

    // returns isize because the return value is dependent on the writer - seems to
    // be usable for anything
    /// Maps to `lua_dump`.
    // pub fn dump<F>(&mut self, mut writer: F, strip: bool) -> c_int
    //     where F: FnMut(&mut State, &[u8]) -> c_int
    //     {
    //         unsafe extern fn write<F>(st: *mut lua_State, p: *const c_void, sz: size_t, ud: *mut c_void) -> c_int
    //             where F: FnMut(&mut State, &[u8]) -> c_int
    //             {
    //                 mem::transmute::<_, &mut F>(ud)(&mut State::from_ptr(st), slice::from_raw_parts(p as *const _, sz as usize))
    //             }
    //         unsafe { lua_dump(self.0, Some(write::<F>), mem::transmute(&mut writer), strip as c_int) }
    //     }

    //===========================================================================
    // Coroutine functions
    //===========================================================================
    /// Maps to `lua_yieldk`.
    pub fn co_yieldk<F>(&mut self, nresults: c_int, continuation: F) -> !
        where F: FnOnce(&mut State, ThreadStatus) -> c_int
        {
            unsafe { lua_yieldk(self.0, nresults, mem::transmute(Box::new(continuation)), Some(continue_func::<F>)) };
            panic!("co_yieldk called in non-coroutine context; check is_yieldable first")
        }

    /// Maps to `lua_yield`. This function is not called `yield` because it is a
    /// reserved keyword.
    pub fn co_yield(&mut self, nresults: c_int) -> ! {
        unsafe { lua_yield(self.0, nresults) };
        panic!("co_yield called in non-coroutine context; check is_yieldable first")
    }

    /// Maps to `lua_resume`.
    pub fn resume(&mut self, from: Option<&mut State>, nargs: c_int) -> ThreadStatus {
        let from_ptr = match from {
            Some(state) => state.0,
            None        => ptr::null_mut()
        };
        let result = unsafe {
            lua_resume(self.0, from_ptr, nargs)
        };
        ThreadStatus::from_c_int(result)
    }

    /// Maps to `lua_status`.
    pub fn status(&mut self) -> ThreadStatus {
        let result = unsafe { lua_status(self.0) };
        ThreadStatus::from_c_int(result)
    }

    /// Maps to `lua_isyieldable`.
    pub fn is_yieldable(&mut self) -> bool {
        let result = unsafe { lua_isyieldable(self.0) };
        result != 0
    }

    //===========================================================================
    // Garbage-collection function
    //===========================================================================
    // TODO: return typing?
    /// Maps to `lua_gc`.
    #[inline]
    pub fn gc(&self, what: GcOption, data: c_int) -> c_int {
        unsafe { lua_gc(self.0, what as c_int, data) }
    }

    //===========================================================================
    // Miscellaneous functions
    //===========================================================================
    /// Maps to `lua_error`.
    pub fn error(&self) -> ! {
        unsafe { lua_error(self.0) };
        unreachable!()
    }

    /// Maps to `lua_next`.
    pub fn next(&self, idx: Index) -> bool {
        let result = unsafe { lua_next(self.0, idx) };
        result != 0
    }

    /// Maps to `lua_concat`.
    #[inline]
    pub fn concat(&self, n: c_int) {
        unsafe { lua_concat(self.0, n) }
    }

    /// Maps to `lua_len`.
    #[inline]
    pub fn len(&self, idx: Index) {
        unsafe { lua_len(self.0, idx) }
    }

    /// Maps to `lua_stringtonumber`.
    pub fn string_to_number(&mut self, s: &str) -> size_t {
        let c_str = CString::new(s).unwrap();
        unsafe { lua_stringtonumber(self.0, c_str.as_ptr()) }
    }

    /// Maps to `lua_getallocf`.
    pub fn get_alloc_fn(&mut self) -> (lua_Alloc, *mut c_void) {
        let mut slot = ptr::null_mut();
        (unsafe { lua_getallocf(self.0, &mut slot) }, slot)
    }

    /// Maps to `lua_setallocf`.
    #[inline]
    pub fn set_alloc_fn(&mut self, f: lua_Alloc, ud: *mut c_void) {
        unsafe { lua_setallocf(self.0, f, ud) }
    }

    //===========================================================================
    // Some useful macros (here implemented as functions)
    //===========================================================================

    /// Set extra data. Return previous value if it was set.
    pub fn set_extra(&mut self, extra: Option<Extra>) -> Option<Extra> {
        self.with_extra(|opt_extra| mem::replace(opt_extra, extra))
    }

    /// Do some actions with mutable extra.
    pub fn with_extra<F, R>(&mut self, closure: F) -> R
        where F: FnOnce(&mut Option<Extra>) -> R {
            unsafe {
                let extra_ptr = lua_getextraspace(self.0) as ExtraHolder;
                let mutex = Box::from_raw(*extra_ptr);
                let result = {
                    let mut guard = mutex.lock().unwrap();
                    closure(guard.deref_mut())
                };
                mem::forget(mutex);
                result
            }
        }

    /// Unwrap and downcast extra to typed.
    ///
    /// # Panics
    ///
    /// Panics if state has no attached `Extra` or it's impossible to downcast to `T`.
    ///
    pub fn with_extra_typed<T, F, R>(&mut self, closure: F) -> R
        where T: any::Any, F: FnOnce(&mut T) -> R {
            self.with_extra(|extra| {
                let data = extra.as_mut().unwrap()
                    .downcast_mut::<T>().unwrap();
                closure(data)
            })
        }

    /// Maps to `lua_tonumber`.
    #[inline]
    pub fn to_number(&self, index: Index) -> lua_Number {
        unsafe { lua_tonumber(self.0, index) }
    }

    /// Maps to `lua_tointeger`.
    #[inline]
    pub fn to_integer(&self, index: Index) -> lua_Integer {
        unsafe { lua_tointeger(self.0, index) }
    }

    /// Maps to `lua_pop`.
    #[inline]
    pub fn pop(&self, n: c_int) {
        unsafe { lua_pop(self.0, n) }
    }

    /// Maps to `lua_newtable`.
    #[inline]
    pub fn new_table(&self) {
        unsafe { lua_newtable(self.0) }
    }

    /// Maps to `lua_register`.
    pub fn register(&self, n: &str, f: lua_CFunction) {
        let c_str = CString::new(n).unwrap();
        unsafe { lua_register(self.0, c_str.as_ptr(), f) }
    }

    /// Maps to `lua_pushcfunction`.
    #[inline]
    pub fn push_fn(&self, f: lua_CFunction) {
        unsafe { lua_pushcfunction(self.0, f) }
    }

    /// Maps to `lua_isfunction`.
    #[inline]
    pub fn is_fn(&mut self, index: Index) -> bool {
        unsafe { lua_isfunction(self.0, index) == 1 }
    }

    /// Maps to `lua_istable`.
    pub fn is_table(&self, index: Index) -> bool {
        unsafe { lua_istable(self.0, index) == 1 }
    }

    /// Maps to `lua_islightuserdata`.
    pub fn is_light_userdata(&self, index: Index) -> bool {
        unsafe { lua_islightuserdata(self.0, index) == 1 }
    }

    /// Maps to `lua_isnil`.
    pub fn is_nil(&self, index: Index) -> bool {
        unsafe { lua_isnil(self.0, index) == 1 }
    }

    /// Maps to `lua_isboolean`.
    pub fn is_bool(&self, index: Index) -> bool {
        unsafe { lua_isboolean(self.0, index) == 1 }
    }

    /// Maps to `lua_isthread`.
    pub fn is_thread(&self, index: Index) -> bool {
        unsafe { lua_isthread(self.0, index) == 1 }
    }

    /// Maps to `lua_isnone`.
    #[inline]
    pub fn is_none(&self, index: Index) -> bool {
        unsafe { lua_isnone(self.0, index) == 1 }
    }

    /// Maps to `lua_isnoneornil`.
    #[inline]
    pub fn is_none_or_nil(&self, index: Index) -> bool {
        unsafe { lua_isnoneornil(self.0, index) == 1 }
    }

    // omitted: lua_pushliteral

    /// Maps to `lua_pushglobaltable`.
    #[inline]
    pub fn push_global_table(&self) {
        unsafe { lua_pushglobaltable(self.0) };
    }

    /// Maps to `lua_insert`.
    #[inline]
    pub fn insert(&self, idx: Index) {
        unsafe { lua_insert(self.0, idx) }
    }

    /// Maps to `lua_remove`.
    #[inline]
    pub fn remove(&self, idx: Index) {
        unsafe { lua_remove(self.0, idx) }
    }

    /// Maps to `lua_replace`.
    #[inline]
    pub fn replace(&self, idx: Index) {
        unsafe { lua_replace(self.0, idx) }
    }

    //===========================================================================
    // Debug API
    //===========================================================================
    /// Maps to `lua_getstack`.
    pub fn get_stack(&self, level: c_int) -> Option<lua_Debug> {
        let mut ar: lua_Debug = unsafe { MaybeUninit::uninit().assume_init() };
        let result = unsafe { lua_getstack(self.0, level, &mut ar) };
        if result == 0 {
            None
        } else {
            Some(ar)
        }
    }

    /// Maps to `lua_getinfo`.
    pub fn get_info(&self, what: &str) -> Option<lua_Debug> {
        let mut ar: lua_Debug = unsafe { MaybeUninit::uninit().assume_init() };
        let c_str = CString::new(what).unwrap();
        let result = unsafe { lua_getinfo(self.0, c_str.as_ptr(), &mut ar) };
        if result == 0 {
            None
        } else {
            Some(ar)
        }
    }

    /// Maps to `lua_getlocal`.
    pub fn get_local(&self, ar: &lua_Debug, n: c_int) -> Option<&str> {
        let ptr = unsafe { lua_getlocal(self.0, ar, n) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
            str::from_utf8(slice).ok()
        }
    }

    /// Maps to `lua_setlocal`.
    pub fn set_local(&self, ar: &lua_Debug, n: c_int) -> Option<&str> {
        let ptr = unsafe { lua_setlocal(self.0, ar, n) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
            str::from_utf8(slice).ok()
        }
    }

    /// Maps to `lua_getupvalue`.
    pub fn get_upvalue(&self, funcindex: Index, n: c_int) -> Option<&str> {
        let ptr = unsafe { lua_getupvalue(self.0, funcindex, n) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
            str::from_utf8(slice).ok()
        }
    }

    /// Maps to `lua_setupvalue`.
    pub fn set_upvalue(&self, funcindex: Index, n: c_int) -> Option<&str> {
        let ptr = unsafe { lua_setupvalue(self.0, funcindex, n) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
            str::from_utf8(slice).ok()
        }
    }

    /// Maps to `lua_upvalueid`.
    pub fn upvalue_id(&self, funcindex: Index, n: c_int) -> *mut c_void {
        unsafe { lua_upvalueid(self.0, funcindex, n) }
    }

    /// Maps to `lua_upvaluejoin`.
    pub fn upvalue_join(&self, fidx1: Index, n1: c_int, fidx2: Index, n2: c_int) {
        unsafe { lua_upvaluejoin(self.0, fidx1, n1, fidx2, n2) }
    }

    /// Maps to `lua_sethook`.
    pub fn set_hook(&self, func: lua_Hook, mask: HookMask, count: c_int) {
        unsafe { lua_sethook(self.0, func, mask.bits(), count) }
    }

    /// Maps to `lua_gethook`.
    pub fn get_hook(&self) -> lua_Hook {
        unsafe { lua_gethook(self.0) }
    }

    /// Maps to `lua_gethookmask`.
    pub fn get_hook_mask(&self) -> HookMask {
        let result = unsafe { lua_gethookmask(self.0) };
        HookMask::from_bits_truncate(result)
    }

    /// Maps to `lua_gethookcount`.
    pub fn get_hook_count(&self) -> c_int {
        unsafe { lua_gethookcount(self.0) }
    }

    //===========================================================================
    // Auxiliary library functions
    //===========================================================================
    /// Maps to `luaL_checkversion`.
    pub fn check_version(&self) {
        unsafe { luaL_checkversion(self.0) }
    }

    /// Maps to `luaL_getmetafield`.
    pub fn get_metafield(&mut self, obj: Index, e: &str) -> bool {
        let c_str = CString::new(e).unwrap();
        let result = unsafe {
            luaL_getmetafield(self.0, obj, c_str.as_ptr())
        };
        result != 0
    }

    /// Maps to `luaL_callmeta`.
    pub fn call_meta(&mut self, obj: Index, e: &str) -> bool {
        let c_str = CString::new(e).unwrap();
        let result = unsafe {
            luaL_callmeta(self.0, obj, c_str.as_ptr())
        };
        result != 0
    }

    /// [-0, +0, -]
    #[inline(always)]
    pub fn to_string(&self, index: Index) -> *const c_char {
        unsafe { lua_tolstring(self.0, index, ptr::null_mut()) }
    }

    /// [-0, +0, -]
    #[inline(always)]
    pub fn tolstring(&self, index: Index, size: &mut usize) -> *const c_char {
        unsafe { lua_tolstring(self.0, index, size as *mut usize) }
    }

    /// Maps to `luaL_tolstring`. This function is not called `to_string` because
    /// that method name is used for the `ToString` trait. This function returns
    /// a reference to the string at the given index, on which `to_owned` may be
    /// called.
    pub fn to_str(&self, index: Index) -> Option<&'static str> {
        let mut len = 0;
        let ptr = self.tolstring(index, &mut len);
        if ptr.is_null() { None } else {
            unsafe {
                let s = slice::from_raw_parts(ptr as *const u8, len as usize);
                Some(str::from_utf8_unchecked(s))
            }
        }
    }

    /// Maps to `lua_tolstring`, but allows arbitrary bytes.
    /// This function returns a reference to the string at the given index,
    /// on which `to_owned` may be called.
    pub fn to_bytes(&self, index: Index) -> Option<&'static [u8]> {
        let mut len = 0;
        let ptr = unsafe { lua_tolstring(self.0, index, &mut len) };
        if ptr.is_null() {
            None
        } else {
            let slice = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
            Some(slice)
        }
    }

    /// Maps to `luaL_argerror`.
    pub fn arg_error(&self, arg: Index, extramsg: &str) -> ! {
        // nb: leaks the CString
        let c_str = CString::new(extramsg).unwrap();
        unsafe { luaL_argerror(self.0, arg, c_str.as_ptr()) };
        unreachable!()
    }

    // omitted: luaL_checkstring
    // omitted: luaL_optstring

    /// Maps to `luaL_checknumber`.
    pub fn check_number(&mut self, arg: Index) -> lua_Number {
        unsafe { luaL_checknumber(self.0, arg) }
    }

    /// Maps to `luaL_optnumber`.
    pub fn opt_number(&mut self, arg: Index, def: lua_Number) -> lua_Number {
        unsafe { luaL_optnumber(self.0, arg, def) }
    }

    /// Maps to `luaL_checkinteger`.
    pub fn check_integer(&mut self, arg: Index) -> lua_Integer {
        unsafe { luaL_checkinteger(self.0, arg) }
    }

    /// Maps to `luaL_optinteger`.
    pub fn opt_integer(&mut self, arg: Index, def: lua_Integer) -> lua_Integer {
        unsafe { luaL_optinteger(self.0, arg, def) }
    }

    /// Maps to `luaL_checkstack`.
    pub fn check_stack_msg(&mut self, sz: c_int, msg: &str) {
        let c_str = CString::new(msg).unwrap();
        unsafe { luaL_checkstack(self.0, sz, c_str.as_ptr()) }
    }

    /// Maps to `luaL_checktype`.
    pub fn check_type(&self, arg: Index, t: Type) {
        unsafe { luaL_checktype(self.0, arg, t as c_int) }
    }

    /// Maps to `luaL_checkany`.
    pub fn check_any(&mut self, arg: Index) {
        unsafe { luaL_checkany(self.0, arg) }
    }

    /// Maps to `luaL_newmetatable`.
    pub fn new_metatable(&mut self, tname: &str) -> bool {
        let c_str = CString::new(tname).unwrap();
        let result = unsafe {
            luaL_newmetatable(self.0, c_str.as_ptr())
        };
        result != 0
    }

    /// Maps to `luaL_setmetatable`.
    pub fn set_metatable_from_registry(&mut self, tname: &str) {
        let c_str = CString::new(tname).unwrap();
        unsafe { luaL_setmetatable(self.0, c_str.as_ptr()) }
    }

    /// Maps to `luaL_testudata`.
    pub fn test_userdata(&mut self, arg: Index, tname: &str) -> *mut c_void {
        let c_str = CString::new(tname).unwrap();
        unsafe { luaL_testudata(self.0, arg, c_str.as_ptr()) }
    }

    /// Convenience function that calls `test_userdata` and performs a cast.
    //#[unstable(reason="this is an experimental function")]
    pub unsafe fn test_userdata_typed<'a, T>(&'a mut self, arg: Index, tname: &str) -> Option<&'a mut T> {
        mem::transmute(self.test_userdata(arg, tname))
    }

    /// Maps to `luaL_checkudata`.
    pub fn check_userdata(&mut self, arg: Index, tname: &str) -> *mut c_void {
        let c_str = CString::new(tname).unwrap();
        unsafe { luaL_checkudata(self.0, arg, c_str.as_ptr()) }
    }

    /// Convenience function that calls `check_userdata` and performs a cast.
    //#[unstable(reason="this is an experimental function")]
    pub unsafe fn check_userdata_typed<'a, T>(&'a mut self, arg: Index, tname: &str) -> &'a mut T {
        mem::transmute(self.check_userdata(arg, tname))
    }

    /// Maps to `luaL_where`. `where` is a reserved keyword.
    pub fn location(&mut self, lvl: c_int) {
        unsafe { luaL_where(self.0, lvl) }
    }

    // omitted: luaL_error

    /// Maps to `luaL_checkoption`.
    pub fn check_option(&mut self, arg: Index, def: Option<&str>, lst: &[&str]) -> usize {
        use std::vec::Vec;
        use libc::c_char;
        let mut vec: Vec<*const c_char> = Vec::with_capacity(lst.len() + 1);
        let cstrs: Vec<CString> = lst.iter().map(|ent| CString::new(*ent).unwrap()).collect();
        for ent in cstrs.iter() {
            vec.push(ent.as_ptr());
        }
        vec.push(ptr::null());
        let result = match def {
            Some(def) => unsafe {
                let c_str = CString::new(def).unwrap();
                luaL_checkoption(self.0, arg, c_str.as_ptr(), vec.as_ptr())
            },
            None      => unsafe {
                luaL_checkoption(self.0, arg, ptr::null(), vec.as_ptr())
            }
        };
        result as usize
    }

    /// Maps to `luaL_fileresult`.
    pub fn file_result(&mut self, stat: c_int, fname: &str) -> c_int {
        let c_str = CString::new(fname).unwrap();
        unsafe { luaL_fileresult(self.0, stat, c_str.as_ptr()) }
    }

    /// Maps to `luaL_execresult`.
    pub fn exec_result(&mut self, stat: c_int) -> c_int {
        unsafe { luaL_execresult(self.0, stat) }
    }

    /// luaL_ref [-1, +0, m]
    #[inline]
    pub fn reference(&self, t: Index) -> Reference {
        let result = unsafe { luaL_ref(self.0, t) };
        Reference(result)
    }

    /// Maps to `luaL_unref`.
    #[inline]
    pub fn unreference(&self, t: Index, reference: Reference) {
        unsafe { luaL_unref(self.0, t, reference.value()) }
    }

    /// Maps to `luaL_loadfilex`.
    pub fn load_filex(&mut self, filename: &str, mode: &str) -> ThreadStatus {
        let result = unsafe {
            let filename_c_str = CString::new(filename).unwrap();
            let mode_c_str = CString::new(mode).unwrap();
            luaL_loadfilex(self.0, filename_c_str.as_ptr(), mode_c_str.as_ptr())
        };
        ThreadStatus::from_c_int(result)
    }

    /// Maps to `luaL_loadfile`.
    pub fn load_file(&mut self, filename: &str) -> ThreadStatus {
        let c_str = CString::new(filename).unwrap();
        let result = unsafe {
            luaL_loadfile(self.0, c_str.as_ptr())
        };
        ThreadStatus::from_c_int(result)
    }

    /// Maps to `luaL_loadbufferx`.
    pub fn load_bufferx(&mut self, buff: &[u8], name: &str, mode: &str) -> ThreadStatus {
        let name_c_str = CString::new(name).unwrap();
        let mode_c_str = CString::new(mode).unwrap();
        let result = unsafe { luaL_loadbufferx(self.0, buff.as_ptr() as *const _, buff.len() as size_t, name_c_str.as_ptr(), mode_c_str.as_ptr()) };
        ThreadStatus::from_c_int(result)
    }

    /// Maps to `luaL_loadstring`.
    pub fn load_string(&mut self, source: &str) -> ThreadStatus {
        let c_str = CString::new(source).unwrap();
        let result = unsafe { luaL_loadstring(self.0, c_str.as_ptr()) };
        ThreadStatus::from_c_int(result)
    }

    // omitted: luaL_newstate (covered by State constructor)

    /// Maps to `luaL_len`.
    pub fn len_direct(&mut self, index: Index) -> lua_Integer {
        unsafe { luaL_len(self.0, index) }
    }

    /// Maps to `luaL_gsub`.
    pub fn gsub(&mut self, s: &str, p: &str, r: &str) -> &str {
        let s_c_str = CString::new(s).unwrap();
        let p_c_str = CString::new(p).unwrap();
        let r_c_str = CString::new(r).unwrap();
        let ptr = unsafe {
            luaL_gsub(self.0, s_c_str.as_ptr(), p_c_str.as_ptr(), r_c_str.as_ptr())
        };
        let slice = unsafe { CStr::from_ptr(ptr).to_bytes() };
        str::from_utf8(slice).unwrap()
    }

    /// Maps to `luaL_setfuncs`.
    pub fn set_fns(&self, l: &[(&str, lua_CFunction)], nup: c_int) {
        use std::vec::Vec;
        let mut reg: Vec<luaL_Reg> = Vec::with_capacity(l.len() + 1);
        let ents: Vec<(CString, lua_CFunction)> = l.iter().map(|&(s, f)| (CString::new(s).unwrap(), f)).collect();
        for &(ref s, f) in ents.iter() {
            reg.push(luaL_Reg {
                name: s.as_ptr(),
                func: f
            });
        }
        reg.push(luaL_Reg {name: ptr::null(), func: None});
        unsafe { luaL_setfuncs(self.0, reg.as_ptr(), nup) }
    }

    /// Maps to `luaL_getsubtable`.
    pub fn get_subtable(&mut self, idx: Index, fname: &str) -> bool {
        let c_str = CString::new(fname).unwrap();
        let result = unsafe {
            luaL_getsubtable(self.0, idx, c_str.as_ptr())
        };
        result != 0
    }

    /// Maps to `luaL_traceback`.
    pub fn traceback(&self, state: &State, msg: &str, level: c_int) {
        let c_str = CString::new(msg).unwrap();
        unsafe { luaL_traceback(self.0, state.0, c_str.as_ptr(), level) }
    }

    /// Maps to `luaL_requiref`.
    pub fn requiref(&self, modname: &str, openf: lua_CFunction, glb: bool) {
        let c_str = CString::new(modname).unwrap();
        unsafe { luaL_requiref(self.0, c_str.as_ptr(), openf, glb as c_int) }
    }

    /// Maps to `luaL_newlibtable`.
    pub fn new_lib_table(&self, l: &[(&str, lua_CFunction)]) {
        self.create_table(0, l.len() as c_int)
    }

    /// Maps to `luaL_newlib`.
    pub fn new_lib(&self, l: &[(&str, lua_CFunction)]) {
        self.check_version();
        self.new_lib_table(l);
        self.set_fns(l, 0)
    }

    /// Maps to `luaL_argcheck`.
    pub fn arg_check(&mut self, cond: bool, arg: Index, extramsg: &str) {
        let c_str = CString::new(extramsg).unwrap();
        unsafe {
            luaL_argcheck(self.0, cond as c_int, arg, c_str.as_ptr())
        }
    }

    /// Maps to `luaL_checklstring`.
    pub fn check_string(&mut self, n: Index) -> &str {
        let mut size = 0;
        let ptr = unsafe { luaL_checklstring(self.0, n, &mut size) };
        let slice = unsafe { slice::from_raw_parts(ptr as *const u8, size as usize) };
        str::from_utf8(slice).unwrap()
    }

    /// Maps to `luaL_optlstring`.
    pub fn opt_string<'a>(&'a mut self, n: Index, default: &'a str) -> &'a str {
        let mut size = 0;
        let c_str = CString::new(default).unwrap();
        let ptr = unsafe { luaL_optlstring(self.0, n, c_str.as_ptr(), &mut size) };
        if ptr == c_str.as_ptr() {
            default
        } else {
            let slice = unsafe { slice::from_raw_parts(ptr as *const u8, size as usize) };
            str::from_utf8(slice).unwrap()
        }
    }

    // omitted: luaL_checkint (use .check_integer)
    // omitted: luaL_optint (use .opt_integer)
    // omitted: luaL_checklong (use .check_integer)
    // omitted: luaL_optlong (use .opt_integer)

    /// Maps to `luaL_typename`.
    pub fn typename_at(&self, n: Index) -> &'static str {
        self.typename_of(self.type_of(n))
    }

    // luaL_dofile and luaL_dostring implemented above

    /// Maps to `luaL_getmetatable`.
    pub fn get_metatable_from_registry(&mut self, tname: &str) {
        let c_str = CString::new(tname).unwrap();
        unsafe { luaL_getmetatable(self.0, c_str.as_ptr()) }
    }

    // omitted: luaL_opt (undocumented function)

    //===========================================================================
    // Wrapper functions
    //===========================================================================
    #[inline]
    pub fn val(&self, i: Index) -> ValRef { ValRef::new(*self, i) }

    /// [-0, +0, -]
    #[inline]
    pub fn upval(&self, i: Index) -> ValRef {
        ValRef::new(*self, lua_upvalueindex(i))
    }

    /// [-0, +0, -]
    #[inline]
    pub fn c_reg(&self) -> Table {
        Table(self.val(LUA_REGISTRYINDEX))
    }

    /// [-0, +1, -]
    #[inline]
    pub fn global(&self) -> Table {
        unsafe { lua_rawgeti(self.0, LUA_REGISTRYINDEX, LUA_RIDX_GLOBALS); }
        Table(self.val(-1))
    }

    pub fn table(&self, narr: c_int, nrec: c_int) -> Table {
        self.create_table(narr, nrec); Table(self.val(-1))
    }

    #[inline]
    pub fn rust_fn(&self, fun: fn(State) -> c_int) -> TopRef {
        unsafe extern "C" fn call_rust_fn(l: *mut lua_State) -> c_int {
            let state = State::from_ptr(l);
            let fp = state.to_pointer(lua_upvalueindex(1));
            let fp: fn(State) -> c_int = mem::transmute(fp);
            fp(state)
        }

        self.push_light_userdata(fun as usize as *mut usize);
        self.push_cclosure(Some(call_rust_fn), 1);
        TopRef(self.val(-1))
    }

    #[inline]
    pub fn method<T>(&self, fun: fn(&mut T, State) -> c_int) -> TopRef {
        self.push_light_userdata(fun as usize as *mut usize);
        self.push_cclosure(Some(Method::<T>::lua_fn), 1);
        TopRef(self.val(-1))
    }


    /// [-0, +1, -]
    pub fn push_userdata<T>(&self, data: T, metatable: Option<InitMetatable>) -> &mut T {
        let result: &mut T = unsafe { mem::transmute(self.new_userdata(mem::size_of::<T>())) };
        mem::forget(mem::replace(result, data));
        if let Some(m) = metatable { self.set_or_init_metatable(m); }
        result
    }

    pub fn load_buffer<F: AsRef<[u8]>>(&self, source: F, chunk_name: Option<&str>) -> Result<ValRef, ThreadStatus> {
        let buffer = source.as_ref();
        let chunk = match chunk_name {
            Some(name) => name.as_ptr(),
            None       => ptr::null(),
        };
        let result = unsafe {
            luaL_loadbuffer(self.0, buffer.as_ptr() as *const c_char, buffer.len(), chunk as *const c_char)
        };
        match result {
            LUA_OK => Ok(self.val(-1)),
            err => Err(ThreadStatus::from_c_int(err)),
        }
    }

    fn get_or_init_metatable(&self, callback: fn(Table, State)) {
        let reg = self.c_reg();
        let p = callback as *const usize;
        let metatable = reg.getp(p);
        if metatable.is_nil() {
            callback(self.table(0, 0), *self);
            assert!(self.type_of(-1) == Type::Table);
            reg.setp(p, self.val(-1));
            self.replace(-2);
        }
    }

    /// [-0, +0, -]
    #[inline]
    pub fn set_or_init_metatable(&self, callback: InitMetatable) {
        let ty = self.type_of(-1);
        assert!(ty == Type::Userdata || ty == Type::Table);
        self.get_or_init_metatable(callback);
        self.set_metatable(-2);
    }

    pub fn rust_closure<F: 'static +  FnMut(State) -> c_int>(&self, closure: F) -> TopRef {
        unsafe extern "C" fn closure_callback(l: *mut lua_State) -> c_int {
            let state = State::from_ptr(l);
            let closure: &mut RustClosure = mem::transmute(
                state.to_userdata(lua_upvalueindex(1))
            );
            (*closure)(state)
        }

        let closure: RustClosure = Box::new(closure);
        let p: &mut RustClosure = unsafe {
            mem::transmute(self.new_userdata(mem::size_of::<RustClosure>()))
        };
        self.set_or_init_metatable(metatable!(
            RustClosure(s: State, this: Self);
            "__gc" () { ptr::drop_in_place(this); 0 }
        ));
        mem::forget(mem::replace(p, closure));
        self.push_cclosure(Some(closure_callback), 1);
        TopRef(self.val(-1))
    }

    /// [-1, +1, -]
    pub fn trace_error(&self, s: Option<&State>) -> &'static str {
        let err = self.to_str(-1).unwrap_or(""); self.pop(1);
        unsafe {
            let thread = s.unwrap_or(self);
            luaL_traceback(self.0, thread.0, err.as_ptr() as *const c_char, 0);
        }
        self.to_str(-1).unwrap_or("")
    }

    #[inline(always)]
    pub fn arg<T: FromLua>(&self, index: Index) -> Option<T> { T::from_lua(self, index) }

    #[inline(always)]
    pub fn args<T: FromLuaMulti>(&self, index: Index) -> T {
        if let Some(args) = T::from_lua(self, index) {
            args
        } else {
            self.push_string("args not match");
            self.error();
        }
    }

    #[inline(always)]
    pub fn fargs<T: FromLuaMulti>(&self) -> T { self.args::<T>(1) }

    #[inline(always)]
    pub fn margs<T: FromLuaMulti>(&self) -> T { self.args::<T>(2) }

    #[inline(always)]
    pub fn to_lua<T: ToLuaMulti>(&self, t: T) -> c_int {
        t.to_lua(self);
        T::COUNT as c_int
    }

    #[inline(always)]
    pub fn pushx<T: ToLuaMulti>(&self, t: T) -> c_int {
        t.to_lua(self);
        T::COUNT as c_int
    }

    #[inline]
    pub fn iter_to_array<T>(&self, iter: impl Iterator<Item = T>, callback: impl Fn(T)) -> i32 {
        let r = self.table(0, 0);
        let mut i = 1;
        for t in iter {
            callback(t);
            r.seti(i, TopRef(r.0)); i += 1;
        }
        1
    }

    #[inline(always)]
    pub fn balance_with<T, F: FnMut(State) -> T>(&self, mut callback: F) -> T {
        let top = self.get_top();
        let result = callback(*self);
        self.set_top(top);
        result
    }

    pub fn raise_error(&self, e: impl std::fmt::Debug) -> ! {
        self.push_string(&format!("{:?}", e)); self.error()
    }

    pub fn value(&self, i: Index) -> Value {
        match unsafe { lua_type(self.0, i) } {
            LUA_TNONE => Value::None,
            LUA_TNIL => Value::Nil,
            LUA_TNUMBER => if self.is_integer(i) {
                Value::Int(self.to_integer(i))
            } else { Value::Num(self.to_number(i)) },
            LUA_TSTRING => Value::Str(self.to_str(i).unwrap()),
            LUA_TBOOLEAN => Value::Bool(self.to_bool(i)),
            LUA_TLIGHTUSERDATA => Value::LightUserdata,
            LUA_TTABLE => Value::Table,
            LUA_TFUNCTION => Value::Function,
            LUA_TUSERDATA => Value::Userdata,
            LUA_TTHREAD => Value::Thread,
            _ => panic!(""),
        }
    }
}