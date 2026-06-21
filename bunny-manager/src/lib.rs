#![feature(once_cell_get_mut)]

use std::{
    env::current_exe,
    ffi::c_void,
    path::{Path, PathBuf},
    sync::OnceLock,
    thread::sleep,
    time::Duration,
};

use anyhow::{Context as _, Result};
use mimalloc::MiMalloc;
use tracing::{error, info};
use windows::Win32::{
    Foundation::{CloseHandle, HINSTANCE, HMODULE},
    System::{
        LibraryLoader::{DisableThreadLibraryCalls, FreeLibraryAndExitThread},
        SystemServices::DLL_PROCESS_ATTACH,
        Threading::{CreateThread, THREAD_CREATION_FLAGS},
    },
};

use crate::{address::find_addresses, egui_hook::ADDRESSES};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub const PLUGINS_PATH: &str = "plugins/bunny_plugins/";
pub const CONFIG_PATH: &str = "plugins/bunny_config/";
pub const FONTS_PATH: &str = "plugins/bunny_fonts/";

pub static EXE_PATH: OnceLock<PathBuf> = OnceLock::new();

mod address;
mod config;
mod egui_hook;
mod hooks;
mod plugin_manager;
mod ui;

fn create_required_dirs(executable_path: impl AsRef<Path>) -> Result<()> {
    let mut base = executable_path.as_ref().to_path_buf();
    base.pop();
    let plugins = base.join(PLUGINS_PATH);
    let config = base.join(CONFIG_PATH);
    let fonts = base.join(FONTS_PATH);
    if !plugins.exists() {
        std::fs::create_dir(&plugins).with_context(|| {
            format!(
                "Failed to create plugins directory at {}",
                plugins.display()
            )
        })?;
        info!("Created plugins directory at {}", plugins.display());
    }
    if !config.exists() {
        std::fs::create_dir(&config).with_context(|| {
            format!("Failed to create config directory at {}", config.display())
        })?;
        info!("Created config directory at {}", config.display());
    }
    if !fonts.exists() {
        std::fs::create_dir(&fonts)
            .with_context(|| format!("Failed to create fonts directory at {}", fonts.display()))?;
        info!("Created fonts directory at {}", fonts.display());
    }
    Ok(())
}

#[allow(static_mut_refs)]
fn fallible() -> Result<()> {
    tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .init();
    let addresses = find_addresses();
    info!(
        "Running {}, found main dll at {:#x}",
        addresses.mhfo_info.game_mode, addresses.mhfo_info.address
    );
    unsafe {
        while (addresses.game_state as *const u8).read() == 0 {
            sleep(Duration::from_millis(100));
        }
    }
    unsafe {
        ADDRESSES
            .set(addresses)
            .expect("ADDRESSES set before startup")
    };

    let exe_path = current_exe()?;
    create_required_dirs(&exe_path)?;
    EXE_PATH.set(exe_path).expect("EXE_PATH set before startup");

    info!("Hooking D3D9");
    egui_hook::hook(addresses.hwnd())?;
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
        error!("{e:#}");
    }
    unsafe {
        FreeLibraryAndExitThread(HMODULE(lp_parameter), 0);
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(hinst: HINSTANCE, fdw_reason: u32, _lpv_reserved: *mut ()) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        unsafe {
            let _ = DisableThreadLibraryCalls(hinst.into());
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
