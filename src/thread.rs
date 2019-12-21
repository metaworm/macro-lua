
use crate::*;

use std::thread;
use std::ptr;
use std::time::Duration;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::os::windows::io::{RawHandle, IntoRawHandle};

struct RustThread {
    state: State,
    r_state: Reference,
    r_udata: Reference,
    recv_end: Receiver<bool>,
    handle: RawHandle,
}

metatable! {
    const METATABLE = RustThread(state: State, this: Self) IndexSelf;

    "handle" () push { this.handle as u64 }
    "__gc" () { 0 }
}

pub(crate) fn init_thread(s: State) {
    let t = s.table(0, 4);
    t.set("spawn", cfunction!((s) {
        s.check_type(1, Type::Function);

        // Init the new state
        let state = s.new_thread();
        let c_reg = s.c_reg();
        s.xmove(state, s.to_lua(s.val(1)));

        // Init the data struct
        let (sender, receiver) = channel::<bool>();
        let thread = s.push_userdata(RustThread {
            state,
            r_state: c_reg.reference(s.val(-1)),
            r_udata: NOREF,
            recv_end: receiver,
            handle: ptr::null_mut(),
        }, Some(METATABLE));
        thread.r_udata = c_reg.reference(s.val(-1));

        // Start the thread
        let r_state = thread.r_state;
        let r_udata = thread.r_udata;
        let h = thread::spawn(move || {
            match state.pcall(0, 0, 0) {
                ThreadStatus::Ok => {}
                ThreadStatus::Yield => {}
                ElseStatus => {}
            }
            state.c_reg().unreference(r_state);
            state.c_reg().unreference(r_udata);
            sender.send(true);
        });
        thread.handle = h.into_raw_handle(); 
        return 1;
    }));

    t.set("sleep", cfunction!((s, time: u64) push {
        thread::sleep(Duration::from_millis(time));
    }));

    t.set("yield_now", cfunction!((s) push {
        thread::yield_now();
    }));
    s.global().set("thread", t.0);
}