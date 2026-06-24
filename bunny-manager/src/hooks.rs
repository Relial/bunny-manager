use anyhow::Result;
use ilhook::x86::{CallbackOption, ClosureHookPoint, HookFlags, hook_closure_jmp_back};
use tracing::{error, info};

use crate::{address::Addresses, egui_hook::APP};

#[allow(static_mut_refs)]
fn hook_game_shutdown<'a>(addresses: Addresses) -> Result<ClosureHookPoint<'a>> {
    let on_call = |_| {
        if let Some(app) = unsafe { APP.get_mut() } {
            let state = app.state_mut();
            if let Err(e) = state.config.save(&state.config_path) {
                error!("Config save error: {e:#}");
            }

            info!("Unloading plugins");
            state.plugin_manager.unload();
            info!("Done unloading");
        }
    };
    let hook = unsafe {
        hook_closure_jmp_back(
            addresses.game_shutdown,
            on_call,
            CallbackOption::None,
            HookFlags::empty(),
        )
    }?;
    Ok(hook)
}

pub fn init<'a>(addresses: &Addresses) -> Result<Vec<ClosureHookPoint<'a>>> {
    Ok(vec![hook_game_shutdown(*addresses)?])
}
