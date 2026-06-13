use std::fmt::Display;
use std::thread::sleep;
use std::time::Duration;

use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::core::s;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub enum GameMode {
    LowGrade,
    HighGrade,
}

impl Display for GameMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GameMode::LowGrade => "Low Grade Edition",
            GameMode::HighGrade => "High Grade Edition",
        };
        write!(f, "{s}")
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MainDllInfo {
    pub game_mode: GameMode,
    pub address: usize,
}

impl MainDllInfo {
    fn new(game_mode: GameMode, address: usize) -> Self {
        Self { game_mode, address }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Addresses {
    pub dll_info: MainDllInfo,
    pub hwnd: usize,
    pub game_state: usize,
    pub game_shutdown: usize,
}

pub fn find_addresses() -> Addresses {
    const SLEEP_DURATION: Duration = Duration::from_millis(100);
    loop {
        if let Ok(handle) = unsafe { GetModuleHandleA(s!("mhfo.dll")) } {
            let dll_info = MainDllInfo::new(GameMode::LowGrade, handle.0.addr());
            return Addresses::new(dll_info);
        } else if let Ok(handle) = unsafe { GetModuleHandleA(s!("mhfo-hd.dll")) } {
            let dll_info = MainDllInfo::new(GameMode::HighGrade, handle.0.addr());
            return Addresses::new(dll_info);
        }
        sleep(SLEEP_DURATION);
    }
}

impl Addresses {
    fn new(main_dll_info: MainDllInfo) -> Self {
        let dll = main_dll_info.address;
        match main_dll_info.game_mode {
            GameMode::LowGrade => Self {
                dll_info: main_dll_info,
                hwnd: dll + 0x5bd9e08,
                game_state: dll + 0x5b460d0,
                game_shutdown: dll + 0x1568a6f,
            },
            GameMode::HighGrade => Self {
                dll_info: main_dll_info,
                hwnd: dll + 0xe811a38,
                game_state: dll + 0xe77dcf8,
                game_shutdown: dll + 0x158f8bf,
            },
        }
    }
}
