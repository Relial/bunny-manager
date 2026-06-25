use anyhow::Result;
use ilhook::x86::{CallbackOption, HookFlags, HookPoint, HookType, Hooker, Registers};
use tracing::{error, info};

use crate::{address::Addresses, egui_hook::APP};

unsafe extern "cdecl" fn on_game_shutdown(_: *mut Registers, _: usize) {
    if let Some(mut app) = APP.get().map(|l| l.lock().unwrap()) {
        let state = app.state_mut();
        if let Err(e) = state.config.save(&state.config_path) {
            error!("Config save error: {e:#}");
        }

        info!("Unloading plugins");
        state.plugin_manager.unload();
        info!("Done unloading");
    }
}

fn hook_game_shutdown(addresses: Addresses) -> Result<HookPoint> {
    let builder = Hooker::new(
        addresses.game_shutdown,
        HookType::JmpBack(on_game_shutdown),
        CallbackOption::None,
        0,
        HookFlags::empty(),
    );
    let hook_point = unsafe { builder.hook() }?;
    Ok(hook_point)
}

pub fn init(addresses: &Addresses) -> Result<Vec<HookPoint>> {
    Ok(vec![hook_game_shutdown(*addresses)?])
}
