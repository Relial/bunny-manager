use std::{ffi::c_void, thread::sleep, time::Duration};

use bunny_plugin::{GameMode, MhfoInfo};
use windows::{
    Win32::{Foundation::HWND, System::LibraryLoader::GetModuleHandleA},
    core::s,
};

#[derive(Clone, Copy, Debug)]
pub struct Addresses {
    pub mhfo_info: MhfoInfo,
    hwnd: usize,
    pub game_state: usize,
    pub lobby_update: usize,
    pub quest_update: usize,
    pub quest_ending_update: usize,
    pub quest_complete_update: usize,
}

pub fn find_addresses() -> Addresses {
    const SLEEP_DURATION: Duration = Duration::from_millis(100);
    loop {
        if let Ok(handle) = unsafe { GetModuleHandleA(s!("mhfo.dll")) } {
            let dll_info = MhfoInfo::new(GameMode::LowGrade, handle.0.addr());
            return Addresses::new(dll_info);
        } else if let Ok(handle) = unsafe { GetModuleHandleA(s!("mhfo-hd.dll")) } {
            let dll_info = MhfoInfo::new(GameMode::HighGrade, handle.0.addr());
            return Addresses::new(dll_info);
        }
        sleep(SLEEP_DURATION);
    }
}

impl Addresses {
    fn new(mhfo_info: MhfoInfo) -> Self {
        let dll = mhfo_info.address;
        match mhfo_info.game_mode {
            GameMode::LowGrade => Self {
                mhfo_info,
                hwnd: dll + 0x5bd9e08,
                game_state: dll + 0x5b460d0,
                lobby_update: dll + 0x882160,
                quest_update: dll + 0x880360,
                quest_ending_update: dll + 0x880cd0,
                quest_complete_update: dll + 0x8810b0,
            },
            GameMode::HighGrade => Self {
                mhfo_info,
                hwnd: dll + 0xe811a38,
                game_state: dll + 0xe77dcf8,
                lobby_update: dll + 0x89dc20,
                quest_update: dll + 0x89be10,
                quest_ending_update: dll + 0x89c780,
                quest_complete_update: dll + 0x89cb50,
            },
        }
    }

    pub fn hwnd(&self) -> HWND {
        let ptr = self.hwnd as *const usize;
        let v = unsafe { ptr.read() };
        HWND(v as *mut c_void)
    }
}
