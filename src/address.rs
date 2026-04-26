use std::fmt::Display;
use std::thread::sleep;
use std::time::Duration;

use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::core::s;

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
pub struct Addresses {
    pub game_mode: GameMode,
    pub dll: usize,
    pub hwnd: usize,
    pub game_state: usize,
    pub game_ending: usize,
}

pub fn find_main_dll() -> Addresses {
    const SLEEP_DURATION: Duration = Duration::from_millis(100);
    loop {
        if let Ok(handle) = unsafe { GetModuleHandleA(s!("mhfo.dll")) } {
            return Addresses::new_lge(handle.0.addr());
        } else if let Ok(handle) = unsafe { GetModuleHandleA(s!("mhfo-hd.dll")) } {
            return Addresses::new_hge(handle.0.addr());
        }
        sleep(SLEEP_DURATION);
    }
}

impl Addresses {
    fn new_lge(dll: usize) -> Self {
        if cfg!(feature = "g1") {
            Self {
                game_mode: GameMode::LowGrade,
                dll,
                hwnd: dll + 0x56f367c,
                game_state: dll + 0x5628e94,
                game_ending: dll + 0xa1435f,
            }
        } else {
            Self {
                game_mode: GameMode::LowGrade,
                dll,
                hwnd: dll + 0x5bd9e08,
                game_state: dll + 0x5b460d0,
                game_ending: dll + 0x1568a6f,
            }
        }
    }

    fn new_hge(dll: usize) -> Self {
        if cfg!(feature = "g1") {
            panic!("G1 plugin loaded with a game version running High Grade Edition");
        } else {
            Self {
                game_mode: GameMode::HighGrade,
                dll,
                hwnd: dll + 0xe811a38,
                game_state: dll + 0xe77dcf8,
                game_ending: dll + 0x158f8bf,
            }
        }
    }
}
