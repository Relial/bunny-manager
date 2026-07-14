use std::{
    mem::transmute,
    sync::{Mutex, OnceLock, atomic::Ordering},
};

use anyhow::{Result, anyhow};
use egui_d3d9::EguiDx9;
use retour::static_detour;
use tracing::debug;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::{
            Direct3D9::{
                D3D_SDK_VERSION, D3DADAPTER_DEFAULT, D3DCREATE_SOFTWARE_VERTEXPROCESSING,
                D3DDEVTYPE_NULLREF, D3DDISPLAYMODE, D3DPRESENT_PARAMETERS, D3DSWAPEFFECT_DISCARD,
                Direct3DCreate9Ex, IDirect3DDevice9,
            },
            Gdi::RGNDATA,
        },
        UI::WindowsAndMessaging::{CallWindowProcW, GWLP_WNDPROC, SetWindowLongPtrA, WNDPROC},
    },
    core::HRESULT,
};

use crate::{
    ADDRESSES,
    ui::ui_manager::{INIT, UiManager},
};

pub static APP: OnceLock<Mutex<EguiDx9<UiManager>>> = OnceLock::new();
static mut O_WND_PROC: Option<WNDPROC> = None;

type FnPresent = unsafe extern "system" fn(
    IDirect3DDevice9,
    *const RECT,
    *const RECT,
    HWND,
    *const RGNDATA,
) -> HRESULT;
type FnReset = unsafe extern "system" fn(IDirect3DDevice9, *const D3DPRESENT_PARAMETERS) -> HRESULT;
static_detour! {
    static PresentHook: unsafe extern "system" fn(IDirect3DDevice9, *const RECT, *const RECT, HWND, *const RGNDATA) -> HRESULT;
    static ResetHook: unsafe extern "system" fn(IDirect3DDevice9, *const D3DPRESENT_PARAMETERS) -> HRESULT;
}

fn hk_present(
    device: IDirect3DDevice9,
    source_rect: *const RECT,
    dest_rect: *const RECT,
    hwnd: HWND,
    dirty_region: *const RGNDATA,
) -> HRESULT {
    unsafe {
        let mut app = APP
            .get_or_init(|| {
                debug!("Initializing EguiDx9");
                let addresses = ADDRESSES
                    .get()
                    .expect("Addresses must be initialized before D3D9 hooks.");
                let hwnd = addresses.hwnd();
                let egui = EguiDx9::init(
                    &device,
                    hwnd,
                    |creation_context| UiManager::new(creation_context, *addresses),
                    false,
                );
                debug!("EguiDx9 initialized. Calling SetWindowLongPtrA");
                O_WND_PROC = Some(transmute::<i32, WNDPROC>(SetWindowLongPtrA(
                    hwnd,
                    GWLP_WNDPROC,
                    hk_wnd_proc as *const () as _,
                )));
                debug!("All init done");
                Mutex::new(egui)
            })
            .lock()
            .unwrap();
        let collect_stats = app.state().collect_stats();
        if collect_stats {
            app.state_mut().stats.frame_start();
        }
        app.present(&device);
        if collect_stats {
            app.state_mut().stats.ui_end();
        }

        let ret = PresentHook.call(device, source_rect, dest_rect, hwnd, dirty_region);
        if collect_stats {
            app.state_mut().stats.frame_end();
        }
        ret
    }
}

fn hk_reset(
    device: IDirect3DDevice9,
    presentation_parameters: *const D3DPRESENT_PARAMETERS,
) -> HRESULT {
    unsafe {
        debug!("hk_reset called");
        if let Some(m) = APP.get() {
            let mut app = m.lock().unwrap();
            app.pre_reset();
            INIT.store(false, Ordering::Release);
        }
        ResetHook.call(device, presentation_parameters)
    }
}

fn hk_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if let Some(m) = APP.get() {
            let mut app = m.lock().unwrap();
            app.wnd_proc(msg, wparam, lparam);
        }
        CallWindowProcW(O_WND_PROC.unwrap(), hwnd, msg, wparam, lparam)
    }
}

pub fn hook(hwnd: HWND) -> Result<()> {
    // Mostly taken from https://github.com/ohchase/shroud
    let direct3d_9 = unsafe { Direct3DCreate9Ex(D3D_SDK_VERSION) }?;
    let mut d3d_display_mode = D3DDISPLAYMODE::default();
    unsafe { direct3d_9.GetAdapterDisplayMode(D3DADAPTER_DEFAULT, &mut d3d_display_mode) }?;

    let mut presentation_parameters = D3DPRESENT_PARAMETERS {
        BackBufferFormat: d3d_display_mode.Format,
        SwapEffect: D3DSWAPEFFECT_DISCARD,
        Windowed: true.into(),
        ..Default::default()
    };
    let mut device = None;
    unsafe {
        direct3d_9.CreateDevice(
            D3DADAPTER_DEFAULT,
            D3DDEVTYPE_NULLREF,
            hwnd,
            D3DCREATE_SOFTWARE_VERTEXPROCESSING as u32,
            &mut presentation_parameters,
            &mut device,
        )?;
    }
    let device = device.ok_or(anyhow!("Failed to create DirectX device"))?;
    let vtable = unsafe {
        std::slice::from_raw_parts(
            transmute::<IDirect3DDevice9, *const *const *const usize>(device).read(),
            119,
        )
    };
    let reset = vtable[16];
    debug!("Found reset at {:X?}", reset);
    let present = vtable[17];
    debug!("Found present at {:X?}", present);
    unsafe {
        let present: FnPresent = transmute(present);
        let reset: FnReset = transmute(reset);

        PresentHook.initialize(present, hk_present)?;
        ResetHook.initialize(reset, hk_reset)?;

        PresentHook.enable()?;
        debug!("Present hooked");
        ResetHook.enable()?;
        debug!("Reset hooked");
    }
    Ok(())
}
