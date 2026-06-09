#![feature(once_cell_get_mut)]

use std::{
    ffi::c_void,
    sync::{Mutex, atomic::Ordering},
    thread::sleep,
    time::Duration,
};

use anyhow::{Result, bail};
use mimalloc::MiMalloc;
use tracing::{error, info, level_filters::LevelFilter};
use windows::Win32::{
    Foundation::{CloseHandle, HINSTANCE, HMODULE, HWND},
    System::{
        LibraryLoader::FreeLibraryAndExitThread,
        SystemServices::DLL_PROCESS_ATTACH,
        Threading::{CreateThread, THREAD_CREATION_FLAGS},
    },
};

use crate::{
    address::find_addresses,
    egui_hook::GAME_HWND,
    plugin_manager::{PLUGIN_MANAGER, PluginManager},
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub const PLUGINS_PATH: &str = "plugins/bunny_plugins/";
pub const CONFIG_PATH: &str = "plugins/bunny_config/";

mod address;
mod config;
mod egui_hook;
mod hooks;
mod plugin_manager;
mod ui;

fn fallible() -> Result<()> {
    tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .init();
    let addresses = find_addresses();
    info!(
        "Running {}, found main dll at {:#x}",
        addresses.dll_info.game_mode, addresses.dll_info.address
    );
    unsafe {
        while (addresses.game_state as *const u8).read() == 0 {
            sleep(Duration::from_millis(100));
        }
    }

    info!("Loading plugins");
    let manager = PluginManager::new(addresses)?;
    info!("Plugin loading done");
    if PLUGIN_MANAGER.set(Mutex::new(manager)).is_err() {
        bail!("Plugin manager already initialized before startup");
    }

    let hwnd_value = unsafe { (addresses.hwnd as *const usize).read() };
    GAME_HWND.store(hwnd_value, Ordering::Relaxed);
    let hwnd = HWND(hwnd_value as *mut c_void);
    info!("Hooking D3D9");
    egui_hook::hook(hwnd)?;
    info!("D3D9 hooks done");

    info!("Hooking game functions");
    let _hooks = hooks::init(&addresses)?;
    info!("Game hooks done");

    const KEEPALIVE: Duration = Duration::from_secs(1);
    loop {
        sleep(KEEPALIVE);
    }
}

extern "system" fn main(lp_parameter: *mut c_void) -> u32 {
    if let Err(e) = fallible() {
        error!("{e}");
    }
    unsafe {
        FreeLibraryAndExitThread(HMODULE(lp_parameter), 0);
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(hinst: HINSTANCE, fdw_reason: u32, _lpv_reserved: *mut ()) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        unsafe {
            if let Ok(handle) = CreateThread(
                None,
                0,
                Some(main),
                Some(hinst.0),
                THREAD_CREATION_FLAGS(0),
                None,
            ) {
                let _ = CloseHandle(handle);
            }
        }
    }
    true
}
