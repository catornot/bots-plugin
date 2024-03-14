use rrplug::{bindings::cvar::convar::FCVAR_CHEAT, prelude::*};
use std::cell::RefCell;

use crate::bindings::ENGINE_FUNCTIONS;

mod detour;
mod random_detour;
mod reversing_detour;

pub static DRAWWORLD_CONVAR: EngineGlobal<RefCell<Option<ConVarStruct>>> =
    EngineGlobal::new(RefCell::new(None));
pub static PAUSABLE_CONVAR: EngineGlobal<RefCell<Option<ConVarStruct>>> =
    EngineGlobal::new(RefCell::new(None));
pub static FORCE_BOX_CONVAR: EngineGlobal<RefCell<Option<ConVarStruct>>> =
    EngineGlobal::new(RefCell::new(None)); // move to dev toys

#[derive(Debug)]
pub struct DevToys;

impl Plugin for DevToys {
    const PLUGIN_INFO: PluginInfo = PluginInfo::new(
        "devtoys\0",
        "devtoys\0",
        "devtoys\0",
        PluginContext::DEDICATED,
    );
    fn new(_: bool) -> Self {
        Self {}
    }

    fn on_dll_load(&self, engine: Option<&EngineData>, dll_ptr: &DLLPointer, token: EngineToken) {
        match dll_ptr.which_dll() {
            WhichDll::Engine => {
                detour::hook_engine(dll_ptr.get_dll_ptr());
                reversing_detour::hook_engine(dll_ptr.get_dll_ptr());
            }
            WhichDll::Server => {
                detour::hook_server(dll_ptr.get_dll_ptr());
                reversing_detour::hook_server(dll_ptr.get_dll_ptr());

                let mut draw_convar = ConVarStruct::find_convar_by_name("r_drawworld", token)
                    .expect("r_drawworld should exist");
                draw_convar.remove_flags(FCVAR_CHEAT as i32, token);

                _ = DRAWWORLD_CONVAR
                    .get(token)
                    .borrow_mut()
                    .replace(draw_convar);

                _ = PAUSABLE_CONVAR.get(token).borrow_mut().replace(
                    ConVarStruct::find_convar_by_name("sv_pausable", token)
                        .expect("sv_pausable should exist"),
                );

                let convar = ConVarStruct::find_convar_by_name("enable_debug_overlays", token)
                    .expect("enable_debug_overlays should exist");
                convar.set_value_i32(1, token);
            }
            WhichDll::Client => random_detour::hook_client(dll_ptr.get_dll_ptr()),

            _ => {}
        }

        let Some(_) = engine else { return };

        let box_convar = ConVarStruct::try_new(
            &ConVarRegister::new(
                "force_mp_box",
                "0",
                0,
                "will put you into mp_box if you are not on mp_box",
            ),
            token,
        )
        .unwrap();

        _ = FORCE_BOX_CONVAR.get(token).replace(Some(box_convar));
    }

    fn runframe(&self, token: EngineToken) {
        match FORCE_BOX_CONVAR.get(token).borrow().as_ref() {
            Some(convar) if convar.get_value_i32() == 1 => {
                let engine = ENGINE_FUNCTIONS.wait();
                let host_state = unsafe {
                    engine
                        .host_state
                        .as_mut()
                        .expect("host state should be valid")
                };

                let level_name = host_state
                    .level_name
                    .iter()
                    .cloned()
                    .filter(|i| *i != 0)
                    .filter_map(|i| char::from_u32(i as u32))
                    .collect::<String>();

                if level_name != "mp_box" {
                    log::info!("go to mp_box. NOW!");

                    unsafe {
                        (engine.cbuf_add_text)(
                            (engine.cbuf_get_current_player)(),
                            "map mp_box\0".as_ptr().cast(),
                            crate::bindings::CmdSource::Code,
                        )
                    };
                    // host_state.next_state = HostState::NewGame;
                    // unsafe { set_c_char_array(&mut host_state.level_name, "mp_box") };
                } else {
                    convar.set_value_i32(0, token)
                }
            }
            None => {}
            Some(_) => {}
        }

        let Ok(convar) = ConVarStruct::find_convar_by_name("idcolor_ally", token) else {
            return;
        };

        let Ok(line) = convar.get_value_str() else {
            return;
        };

        let Some(color) = line.split(' ').next() else {
            return;
        };

        let Ok(value) = color.parse::<f32>() else {
            return;
        };

        convar.set_value_string(
            format!(
                "{:.*} 0.100 1.000 8",
                3,
                if value < 1. { value + 0.01 } else { 0. }
            ),
            token,
        )
    }
}