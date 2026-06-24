use std::{
    ffi::{OsString, c_void},
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    str::FromStr,
    sync::OnceLock,
    thread::sleep,
    time::Duration,
};

use anyhow::{Context as _, Result, anyhow};
use bunny_plugin::LogLevel;
use mimalloc::MiMalloc;
use tracing::{debug, error, info};
use windows::Win32::{
    Foundation::{CloseHandle, HINSTANCE, HMODULE},
    System::{
        LibraryLoader::{DisableThreadLibraryCalls, FreeLibraryAndExitThread, GetModuleFileNameW},
        SystemServices::DLL_PROCESS_ATTACH,
        Threading::{CreateThread, THREAD_CREATION_FLAGS},
    },
};

use crate::address::{Addresses, find_addresses};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub const PLUGINS_DIR_NAME: &str = "bunny_plugins";
pub const CONFIG_DIR_NAME: &str = "bunny_config";
pub const FONTS_DIR_NAME: &str = "bunny_fonts";

pub static MODULE_DIR_PATH: OnceLock<PathBuf> = OnceLock::new();
pub static LOG_LEVEL: OnceLock<LogLevel> = OnceLock::new();
pub static ADDRESSES: OnceLock<Addresses> = OnceLock::new();

mod address;
mod config;
mod egui_hook;
mod hooks;
mod plugin_manager;
mod ui;

fn get_own_dir(module: HMODULE) -> Result<PathBuf> {
    let mut buf = [0; 1024];
    let return_len = unsafe { GetModuleFileNameW(Some(module), &mut buf) };
    let err = std::io::Error::last_os_error();
    if return_len == 0 {
        Err(anyhow!("Failed to get own module path: {err:#}"))
    } else {
        let s = OsString::from_wide(&buf[0..return_len as usize]);
        let mut p = PathBuf::from(s);
        p.pop();
        debug!("Module directory: {}", p.display());
        Ok(p)
    }
}

fn create_required_dirs(module_dir: impl AsRef<Path>) -> Result<()> {
    let base = module_dir.as_ref();
    let plugins = base.join(PLUGINS_DIR_NAME);
    let config = base.join(CONFIG_DIR_NAME);
    let fonts = base.join(FONTS_DIR_NAME);
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

fn fallible(module: HMODULE) -> Result<()> {
    let (log_level, log_level_error) = match std::env::var("CARDAMOM_LOG_LEVEL") {
        Ok(level_str) => match LogLevel::from_str(&level_str) {
            Ok(level) => (level, None),
            Err(e) => (
                LogLevel::default(),
                Some(e.context(
                    "Failed to parse CARDAMOM_LOG_LEVEL environment variable as LogLevel struct",
                )),
            ),
        },
        Err(e) => {
            let err = match e {
                std::env::VarError::NotPresent => {
                    anyhow!("Environment variable CARDADMOM_LOG_LEVEL not found")
                }
                std::env::VarError::NotUnicode(_) => {
                    anyhow!("Environment variable CARDAMOM_LOG_LEVEL is invalid Unicode")
                }
            };
            (LogLevel::default(), Some(err))
        }
    };
    tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(log_level)
        .init();
    if let Some(e) = log_level_error {
        error!("{e:#}");
    }
    LOG_LEVEL
        .set(log_level)
        .expect("LOG_LEVEL set before startup");

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

    ADDRESSES
        .set(addresses)
        .expect("ADDRESSES set before startup");

    let module_dir_path = get_own_dir(module)?;
    create_required_dirs(&module_dir_path)?;
    MODULE_DIR_PATH
        .set(module_dir_path)
        .expect("EXE_PATH set before startup");

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
    let module = HMODULE(lp_parameter);
    if let Err(e) = fallible(module) {
        error!("{e:#}");
    }
    unsafe {
        FreeLibraryAndExitThread(module, 0);
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
