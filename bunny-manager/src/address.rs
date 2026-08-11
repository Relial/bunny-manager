use std::{ffi::c_void, thread::sleep, time::Duration};

use anyhow::Result;
use bunny_plugin::{GameMode, MhfoInfo, bunny_3d::Matrix4x4};
use egui::{Pos2, Rect};
use glam::Mat4;
use windows::{
    Win32::{
        self, Foundation::HWND, Graphics::Direct3D9::IDirect3DDevice9,
        System::LibraryLoader::GetModuleHandleA, UI::WindowsAndMessaging::GetClientRect,
    },
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
    view_matrix: usize,
    projection_matrix: usize,
    pub rendering_stuff: usize,
    d3d_device: usize,
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
                view_matrix: dll + 0x5c47360,
                projection_matrix: dll + 0x5c47320,
                rendering_stuff: dll + 0xb5c630,
                d3d_device: dll + 0x5bd9e0c,
            },
            GameMode::HighGrade => Self {
                mhfo_info,
                hwnd: dll + 0xe811a38,
                game_state: dll + 0xe77dcf8,
                lobby_update: dll + 0x89dc20,
                quest_update: dll + 0x89be10,
                quest_ending_update: dll + 0x89c780,
                quest_complete_update: dll + 0x89cb50,
                view_matrix: dll + 0xe87ef90,
                projection_matrix: dll + 0xe87ef50,
                rendering_stuff: dll + 0xb7af3a,
                d3d_device: dll + 0xe811a3c,
            },
        }
    }

    #[inline]
    pub fn hwnd(&self) -> HWND {
        let ptr = self.hwnd as *const usize;
        let v = unsafe { ptr.read() };
        HWND(v as *mut c_void)
    }

    #[inline]
    pub fn view_matrix(&self) -> Matrix4x4 {
        unsafe { (self.view_matrix as *const Matrix4x4).read() }
    }

    #[inline]
    pub fn view_glam(&self) -> Mat4 {
        unsafe { (self.view_matrix as *const Mat4).read() }
    }

    #[inline]
    pub fn projection_matrix(&self) -> Matrix4x4 {
        unsafe { (self.projection_matrix as *const Matrix4x4).read() }
    }

    #[inline]
    pub fn proj_glam(&self) -> Mat4 {
        unsafe { (self.projection_matrix as *const Mat4).read() }
    }

    pub fn d3d9_device(&self) -> *const IDirect3DDevice9 {
        match self.mhfo_info.game_mode {
            GameMode::LowGrade => {
                let proxy = unsafe { (self.d3d_device as *const *const u8).read() };
                proxy.wrapping_byte_add(0xc) as *const IDirect3DDevice9
            }
            GameMode::HighGrade => self.d3d_device as *const IDirect3DDevice9,
        }
    }

    pub fn get_client_rect(&self) -> Result<Rect> {
        let mut rect: Win32::Foundation::RECT = Default::default();
        unsafe { GetClientRect(self.hwnd(), &mut rect) }?;
        Ok(Rect::from_min_max(
            Pos2 {
                x: rect.left as f32,
                y: rect.top as f32,
            },
            Pos2 {
                x: rect.right as f32,
                y: rect.bottom as f32,
            },
        ))
    }
}
