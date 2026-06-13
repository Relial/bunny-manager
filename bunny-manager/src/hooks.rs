use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use crate::{address::Addresses, egui_hook::APP, plugin_manager::PLUGIN_MANAGER};

use anyhow::Result;
use ilhook::x86::{CallbackOption, ClosureHookPoint, HookFlags, hook_closure_jmp_back};
use tracing::{error, info};

#[allow(static_mut_refs)]
fn hook_game_shutdown<'a>(addresses: Addresses) -> Result<ClosureHookPoint<'a>> {
    const SAVE_TIMEOUT: Duration = Duration::from_secs(5);

    let on_call = |_| {
        if let Some(mut manager) = PLUGIN_MANAGER.get().map(|mutex| mutex.lock().unwrap()) {
            info!("Saving configs");
            let mut handles: Vec<_> = manager.plugins.iter().flat_map(|p| p.save()).collect();
            if let Some(app) = unsafe { APP.get() } {
                let state = app.state();
                let config_path = state.config_path.clone();
                let config = state.config;
                let handle = std::thread::spawn(move || {
                    if let Some(path) = &config_path
                        && let Err(e) = config.save(path)
                    {
                        error!("Config save error: {e}");
                    }
                });
                handles.push(handle);
            }

            let save_start = Instant::now();
            while save_start.elapsed() < SAVE_TIMEOUT {
                if handles.iter().all(|h| h.is_finished()) {
                    break;
                }
                sleep(Duration::from_millis(100));
            }

            info!("Running plugin unload funcs");
            for plugin in &mut manager.plugins {
                plugin.unload();
            }
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
