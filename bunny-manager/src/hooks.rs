use anyhow::Result;
use bunny_plugin::hook::HookKind;
use ilhook::x86::{CallbackOption, HookFlags, HookPoint, HookType, Hooker, Registers};
use tracing::debug;

use crate::{address::Addresses, egui_hook::APP};

unsafe extern "cdecl" fn on_lobby_update(_: *mut Registers, _: usize) {
    if let Some(m) = APP.get() {
        let app = m.lock().unwrap();
        let plugin_manager = &app.state().plugin_manager;
        plugin_manager.run_hook_callbacks(HookKind::Lobby);
    }
}

fn hook_lobby_update(addresses: &Addresses) -> Result<HookPoint> {
    let hook_address = addresses.lobby_update;
    let builder = Hooker::new(
        hook_address,
        HookType::JmpBack(on_lobby_update),
        CallbackOption::None,
        0,
        HookFlags::empty(),
    );
    let hook_point = unsafe { builder.hook() }?;
    debug!("Hooked lobby update at {:#x}", hook_address);
    Ok(hook_point)
}

unsafe extern "cdecl" fn on_quest_update(_: *mut Registers, _: usize) {
    if let Some(m) = APP.get() {
        let app = m.lock().unwrap();
        let plugin_manager = &app.state().plugin_manager;
        plugin_manager.run_hook_callbacks(HookKind::Quest);
    }
}

fn hook_quest_update(addresses: &Addresses) -> Result<HookPoint> {
    let hook_address = addresses.quest_update;
    let builder = Hooker::new(
        hook_address,
        HookType::JmpBack(on_quest_update),
        CallbackOption::None,
        0,
        HookFlags::empty(),
    );
    let hook_point = unsafe { builder.hook() }?;
    debug!("Hooked quest update at {:#x}", hook_address);
    Ok(hook_point)
}

unsafe extern "cdecl" fn on_quest_ending_update(_: *mut Registers, _: usize) {
    if let Some(m) = APP.get() {
        let app = m.lock().unwrap();
        let plugin_manager = &app.state().plugin_manager;
        plugin_manager.run_hook_callbacks(HookKind::QuestEnding);
    }
}

fn hook_quest_ending_update(addresses: &Addresses) -> Result<HookPoint> {
    let hook_address = addresses.quest_ending_update;
    let builder = Hooker::new(
        hook_address,
        HookType::JmpBack(on_quest_ending_update),
        CallbackOption::None,
        0,
        HookFlags::empty(),
    );
    let hook_point = unsafe { builder.hook() }?;
    debug!("Hooked quest ending update at {:#x}", hook_address);
    Ok(hook_point)
}

unsafe extern "cdecl" fn on_quest_complete_update(_: *mut Registers, _: usize) {
    if let Some(m) = APP.get() {
        let app = m.lock().unwrap();
        let plugin_manager = &app.state().plugin_manager;
        plugin_manager.run_hook_callbacks(HookKind::QuestComplete);
    }
}

fn hook_quest_complete_update(addresses: &Addresses) -> Result<HookPoint> {
    let hook_address = addresses.quest_complete_update;
    let builder = Hooker::new(
        hook_address,
        HookType::JmpBack(on_quest_complete_update),
        CallbackOption::None,
        0,
        HookFlags::empty(),
    );
    let hook_point = unsafe { builder.hook() }?;
    debug!("Hooked quest complete update update at {:#x}", hook_address);
    Ok(hook_point)
}

pub fn init(addresses: &Addresses) -> Result<Vec<HookPoint>> {
    Ok(vec![
        hook_lobby_update(addresses)?,
        hook_quest_update(addresses)?,
        hook_quest_ending_update(addresses)?,
        hook_quest_complete_update(addresses)?,
    ])
}
