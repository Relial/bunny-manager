use std::{ffi::c_void, sync::atomic::Ordering, thread::sleep, time::Duration};

use anyhow::Result;
use mimalloc::MiMalloc;
use tracing::{error, info};
use windows::Win32::{
    Foundation::{CloseHandle, HINSTANCE, HMODULE, HWND},
    System::{
        LibraryLoader::FreeLibraryAndExitThread,
        SystemServices::DLL_PROCESS_ATTACH,
        Threading::{CreateThread, THREAD_CREATION_FLAGS},
    },
};

use crate::{
    address::find_main_dll,
    egui_hook::GAME_HWND,
    plugins::{PluginDirs, initialize_plugins, load_plugins},
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod address;
mod egui_hook;
mod hook;
mod plugins;
mod ui;

fn fallible() -> Result<()> {
    tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .init();
    let addresses = find_main_dll();
    info!(
        "Running {}, found main dll at {:#x}",
        addresses.game_mode, addresses.dll
    );
    unsafe {
        while (addresses.game_state as *const u8).read() == 0 {
            sleep(Duration::from_millis(100));
        }
    }
    let hwnd_value = unsafe { (addresses.hwnd as *const usize).read() };
    GAME_HWND.store(hwnd_value, Ordering::Relaxed);
    let hwnd = HWND(hwnd_value as *mut c_void);
    info!("Initializing D3D9 hooks");
    egui_hook::hook(hwnd)?;
    info!("D3D9 hooks enabled successfully");

    info!("Hooking game functions");
    let _hooks = hook::init(&addresses)?;
    info!("Finished hooking");

    info!("Loading plugins");
    let plugin_dirs = PluginDirs::new()?;
    load_plugins(&plugin_dirs.plugins)?;
    info!("Finished loading");
    info!("Initializing plugins");
    initialize_plugins(&plugin_dirs.configs, addresses.dll, addresses.game_mode);
    info!("Finished initialization");

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
