use libloading::{
    library_filename,
    os::windows::{Library, LOAD_WITH_ALTERED_SEARCH_PATH},
};
use once_cell::sync::Lazy;
use std::{
    ffi::{c_char, c_int, c_void},
    mem::{self, transmute},
    path::PathBuf,
};

use crate::{
    hooks::{DllHook, FuncHooks},
    screen_detour::hook_materialsystem,
    structs::{
        cbaseclient::CbaseClientPtr,
        cbaseplayer::CbasePlayerPtr,
        clientarray::{ClientArray, ClientArrayPtr},
    },
};

static EXE_DIR: Lazy<PathBuf> = Lazy::new(|| {
    std::env::current_exe()
        .expect("Unable to get the path to the executable")
        .parent()
        .expect("Executable path has no parent dir")
        .to_path_buf()
});

pub type PServer = *const c_void;
pub type BotName = *const c_char;
pub type ServerGameClients = *const c_void;
pub type PlayerByIndex = unsafe extern "fastcall" fn(i32) -> CbasePlayerPtr;
pub type ClientFullyConnected = unsafe extern "fastcall" fn(ServerGameClients, u16, bool);
pub type RunNullCommand = unsafe extern "fastcall" fn(CbasePlayerPtr);
pub type CreateFakeClient = unsafe extern "fastcall" fn(
    PServer,
    BotName,
    *const c_char,
    *const c_char,
    i32,
    i32,
) -> CbaseClientPtr;

pub struct SourceEngineData {
    pub server: PServer,
    pub game_clients: ServerGameClients,
    pub create_fake_client: CreateFakeClient,
    pub client_fully_connected: ClientFullyConnected,
    pub run_null_command: RunNullCommand,
    pub client_array: ClientArray,
    pub player_by_index: PlayerByIndex,
    pub some_ctexture_function: unsafe extern "C" fn(*const c_void, c_int) -> i16,
}

impl std::fmt::Debug for SourceEngineData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceEngineData")
            .field("server", &self.server)
            .field("game_clients", &self.game_clients)
            .field(
                "create_fake_client",
                &(self.create_fake_client as *const c_void),
            )
            .field(
                "client_fully_connected",
                &(self.client_fully_connected as *const c_void),
            )
            .field(
                "run_null_command",
                &(self.run_null_command as *const c_void),
            )
            .field("client_array", &self.client_array)
            .field("player_by_index", &(self.player_by_index as *const c_void))
            .finish()
    }
}

unsafe impl Send for SourceEngineData {}

impl SourceEngineData {
    pub fn load_server(&mut self, hook_functions: FuncHooks) {
        let path = EXE_DIR.clone().join(library_filename("server"));

        log::info!("loading server.dll from path {}", path.display());

        let handle_server = match unsafe { Library::load_with_flags(path, 0) } {
            Ok(lib) => lib.into_raw() as *const c_void,
            Err(err) => {
                log::error!("{err}");
                return;
            }
        };

        self.client_fully_connected = unsafe { mem::transmute(handle_server.offset(0x153B70)) };
        self.run_null_command = unsafe { mem::transmute(handle_server.offset(0x5A9FD0)) };
        self.player_by_index = unsafe { mem::transmute(handle_server.offset(0x26AA10)) };

        let dll_hook = DllHook::new(handle_server);
        for boxed_hook_struct in hook_functions.iter() {
            boxed_hook_struct.hook_server(&dll_hook);
        }

        if let Err(err) = unsafe { Library::from_raw(handle_server as *mut _).close() } {
            log::error!("couldn't close the handle_engine; {err}")
        }
    }

    pub fn load_engine(&mut self, hook_functions: FuncHooks) {
        let path = EXE_DIR
            .clone()
            .join("bin")
            .join("x64_retail")
            .join("engine.dll");

        log::info!("loading engine.dll from path {}", path.display());

        let handle_engine =
            match unsafe { Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) } {
                Ok(lib) => lib.into_raw() as *const c_void,
                Err(err) => {
                    log::error!("{err}");
                    return;
                }
            };

        let dll_hook = DllHook::new(handle_engine);
        for boxed_hook_struct in hook_functions.iter() {
            boxed_hook_struct.hook_engine(&dll_hook);
        }

        unsafe {
            self.server = handle_engine.offset(0x12A53D40) as PServer;
            self.game_clients = handle_engine.offset(0x13F0AAA8) as ServerGameClients;
            self.create_fake_client = mem::transmute(handle_engine.offset(0x114C60));
            self.client_array =
                ClientArray::new(handle_engine.offset(0x12A53F90) as ClientArrayPtr);
        }

        if let Err(err) = unsafe { Library::from_raw(handle_engine as *mut _).close() } {
            log::error!("couldn't close the handle_engine; {err}")
        }
    }

    pub fn load_materialsystem(&mut self) {
        let path = EXE_DIR
            .clone()
            .join("bin")
            .join("x64_retail")
            .join("materialsystem_dx11.dll");

        log::info!(
            "loading materialsystem_dx11.dll from path {}",
            path.display()
        );

        let handle_matsys =
            match unsafe { Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) } {
                Ok(lib) => lib.into_raw() as *const c_void,
                Err(err) => {
                    log::error!("{err}");
                    return;
                }
            };

        log::info!("base matsys addr: {handle_matsys:?}");

        hook_materialsystem(handle_matsys);

        unsafe {
            self.some_ctexture_function = transmute(handle_matsys as usize + 0x00079e80);
        }

        if let Err(err) = unsafe { Library::from_raw(handle_matsys as *mut _).close() } {
            log::error!("couldn't close the handle_engine; {err}")
        }
    }

    pub fn load_client(&mut self, hook_functions: FuncHooks) {
        let path = EXE_DIR
            .clone()
            .join("bin")
            .join("x64_retail")
            .join("client.dll");

        log::info!("loading client.dll from path {}", path.display());

        let client_handle =
            match unsafe { Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) } {
                Ok(lib) => lib.into_raw() as *const c_void,
                Err(err) => {
                    log::error!("{err}");
                    return;
                }
            };

        let dll_hook = DllHook::new(client_handle);
        for boxed_hook_struct in hook_functions.iter() {
            boxed_hook_struct.hook_client(&dll_hook);
        }

        if let Err(err) = unsafe { Library::from_raw(client_handle as *mut _).close() } {
            log::error!("couldn't close the handle_engine; {err}")
        }
    }
}
