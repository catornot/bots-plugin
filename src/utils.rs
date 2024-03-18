use rrplug::{bindings::class_types::cplayer::CPlayer, mid::utils::try_cstring, prelude::*};
use std::{
    ffi::{c_char, c_void, CStr},
    marker::PhantomData,
};

use windows_sys::Win32::System::{
    Diagnostics::Debug::WriteProcessMemory,
    LibraryLoader::{GetModuleHandleA, GetProcAddress},
    Threading::GetCurrentProcess,
};

use crate::{bindings::ENGINE_FUNCTIONS, interfaces::ENGINE_INTERFACES};

pub struct Pointer<'a, T> {
    pub ptr: *const T,
    marker: PhantomData<&'a T>,
}

impl<'a, T> From<*const T> for Pointer<'a, T> {
    fn from(value: *const T) -> Self {
        Self {
            ptr: value,
            marker: PhantomData,
        }
    }
}

impl<'a, T> From<*mut T> for Pointer<'a, T> {
    fn from(value: *mut T) -> Self {
        Self {
            ptr: value.cast_const(),
            marker: PhantomData,
        }
    }
}

impl<'a, T> From<Pointer<'a, T>> for *const T {
    fn from(val: Pointer<'a, T>) -> Self {
        val.ptr
    }
}

impl<'a, T> From<Pointer<'a, T>> for *mut T {
    fn from(val: Pointer<'a, T>) -> Self {
        val.ptr.cast_mut()
    }
}

#[inline]
pub(crate) unsafe fn iterate_c_array_sized<T, const U: usize>(
    ptr: Pointer<T>,
) -> impl Iterator<Item = &T> {
    let ptr: *const T = ptr.into();
    (0..U).filter_map(move |i| ptr.add(i).as_ref())
}

#[inline]
pub(crate) unsafe fn set_c_char_array<const U: usize>(buf: &mut [c_char; U], new: &str) {
    *buf = [0; U]; // null everything
    buf.iter_mut()
        .zip(new.as_bytes())
        .for_each(|(buf_char, new)| *buf_char = *new as i8);
    buf[U - 1] = 0; // also null last byte
}

#[inline]
pub(crate) unsafe fn from_c_string<T: From<String>>(ptr: *const c_char) -> T {
    CStr::from_ptr(ptr).to_string_lossy().to_string().into()
}

#[allow(unused)]
#[inline]
pub(crate) unsafe fn patch(addr: usize, bytes: &[u8]) {
    WriteProcessMemory(
        GetCurrentProcess(),
        addr as *const c_void,
        bytes as *const _ as *const c_void,
        bytes.len(),
        std::ptr::null_mut(),
    );
}

pub(crate) fn send_client_print(player: &CPlayer, msg: &str) -> Option<()> {
    let engine = ENGINE_FUNCTIONS.wait();

    let client = unsafe {
        engine
            .client_array
            .add(player.player_index.copy_inner() as usize - 1)
            .as_ref()?
    };
    let msg = try_cstring(msg).ok()?;

    unsafe { (engine.cgame_client_printf)(client, msg.as_ptr()) };

    None
}

pub(crate) unsafe fn client_command(edict: u16, command: *const c_char) {
    const ZERO_STRING: *const c_char = "\0".as_ptr() as *const _;

    let func = ENGINE_INTERFACES
        .get()
        .unwrap_unchecked()
        .engine_server
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap()[23];
    let client_command: unsafe extern "C" fn(
        *const c_void,
        *const u16,
        *const c_char,
        *const c_char,
    ) = std::mem::transmute(func);

    client_command(std::ptr::null(), &edict, command, ZERO_STRING);
}

#[allow(unused)]
pub(crate) unsafe fn server_command(command: *const c_char) {
    let func = ENGINE_INTERFACES
        .get()
        .unwrap_unchecked()
        .engine_server
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap()[21];
    let client_command: unsafe extern "C" fn(*const c_void, *const c_char) =
        std::mem::transmute(func);

    client_command(std::ptr::null(), command);
}

pub(crate) unsafe fn create_source_interface<T>(
    module: *const c_char,
    interface: *const c_char,
) -> Option<&'static mut T> {
    const CREATEINTERFACE: *const u8 = "CreateInterface\0".as_ptr() as *const _;

    let module = GetModuleHandleA(module as *const _);

    if module == 0 {
        log::error!("failed to get interface");
        return None;
    }

    let create_interface_func: extern "C" fn(*const c_char, *const i32) -> *mut T =
        std::mem::transmute(GetProcAddress(module, CREATEINTERFACE));

    return create_interface_func(interface, std::ptr::null()).as_mut();
}
