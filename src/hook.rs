use crate::{address::Addresses, plugins::PLUGINS};

use anyhow::Result;
use ilhook::x86::{CallbackOption, ClosureHookPoint, HookFlags, hook_closure_jmp_back};
use tracing::info;

fn hook_game_ending<'a>(addresses: Addresses) -> Result<ClosureHookPoint<'a>> {
    let on_call = |_| {
        if let Some(plugins) = PLUGINS.lock().unwrap().as_ref() {
            info!("Running plugin unload funcs");
            for plugin in plugins {
                unsafe { (plugin.funcs.unload)() }
            }
            info!("Finished unload funcs");
        }
    };
    let hook = unsafe {
        hook_closure_jmp_back(
            addresses.game_ending,
            on_call,
            CallbackOption::None,
            HookFlags::empty(),
        )
    }?;
    Ok(hook)
}

pub fn init<'a>(addresses: &Addresses) -> Result<Vec<ClosureHookPoint<'a>>> {
    Ok(vec![hook_game_ending(*addresses)?])
}
