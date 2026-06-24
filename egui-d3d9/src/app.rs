use clipboard::{windows_clipboard::WindowsClipboardContext, ClipboardProvider};
use egui::{epaint::Primitive, Context, Ui};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    Graphics::Direct3D9::{IDirect3DDevice9, D3DPT_TRIANGLELIST, D3DVIEWPORT9},
    UI::WindowsAndMessaging::GetClientRect,
};

use crate::{
    inputman::InputManager,
    mesh::{Buffers, GpuVertex, MeshDescriptor},
    state::DxState,
    texman::TextureManager,
};

pub trait App {
    fn ui(&mut self, ui: &mut Ui);
}

unsafe impl<T: App> Send for EguiDx9<T> {}

pub struct EguiDx9<T: App> {
    ui_state: T,
    hwnd: HWND,
    reactive: bool,
    input_man: InputManager,
    // get it? tEx-man? tax-man? no?
    tex_man: TextureManager,
    ctx: Context,
    buffers: Buffers,
    prims: Vec<MeshDescriptor>,
    last_idx_capacity: usize,
    last_vtx_capacity: usize,
    should_reset: bool,
    skip_frame: u8,
}

impl<T: App> EguiDx9<T> {
    ///
    /// initialize the backend.
    ///
    ///
    /// if you are using this purely as a UI, you can set `reactive` to true.
    /// this causes us to only re-draw the menu once something changes.
    ///
    /// the menu doesn't always catch these changes, so only use this if you need to.
    ///
    pub fn init(
        dev: &IDirect3DDevice9,
        hwnd: HWND,
        ui_create: impl FnOnce(&Context) -> T,
        reactive: bool,
    ) -> Self {
        if hwnd.is_invalid() {
            panic!("invalid hwnd specified in egui init");
        }

        let ctx = Context::default();
        let ui_state = ui_create(&ctx);

        Self {
            ui_state,
            hwnd,
            reactive,
            tex_man: TextureManager::new(),
            input_man: InputManager::new(hwnd),
            ctx: Context::default(),
            buffers: Buffers::create_buffers(dev, 16384, 16384),
            prims: Vec::new(),
            last_idx_capacity: 0,
            last_vtx_capacity: 0,
            should_reset: false,
            skip_frame: 2,
        }
    }

    pub fn pre_reset(&mut self) {
        self.buffers.delete_buffers();
        self.tex_man.deallocate_textures();

        self.should_reset = true;
        self.skip_frame = 2;
    }

    pub fn present(&mut self, dev: &IDirect3DDevice9) {
        if unsafe { dev.TestCooperativeLevel() }.is_err() {
            return;
        }
        if self.skip_frame > 0 {
            if self.should_reset {
                self.buffers = Buffers::create_buffers(dev, 16384, 16384);
                self.prims.clear();
                self.last_idx_capacity = 0;
                self.last_vtx_capacity = 0;
                self.should_reset = false;

                let memory = self.ctx.memory(|mem| mem.clone());
                self.ctx = Context::default();
                self.ctx.memory_mut(|mem| *mem = memory);
            }
            self.skip_frame -= 1;
            return;
        }

        let output = self.ctx.run_ui(self.input_man.collect_input(), |ui| {
            // safe. present will never run in parallel.
            self.ui_state.ui(ui);
        });

        if !output.textures_delta.is_empty() {
            self.tex_man.process_set_deltas(dev, &output.textures_delta);
        }

        if output.shapes.is_empty() {
            // early return, don't forget to free textures
            if !output.textures_delta.is_empty() {
                self.tex_man.process_free_deltas(&output.textures_delta);
            }
            return;
        }

        // we only need to update the buffers if we are actually changing something
        if self.ctx.has_requested_repaint() || !self.reactive {
            let mut vertices: Vec<GpuVertex> = Vec::with_capacity(self.last_vtx_capacity + 512);
            let mut indices: Vec<u32> = Vec::with_capacity(self.last_idx_capacity + 512);

            self.prims = self
                .ctx
                .tessellate(output.shapes, output.pixels_per_point)
                .into_iter()
                .filter_map(|prim| {
                    if let Primitive::Mesh(mesh) = prim.primitive {
                        // most definitely not the rusty way to do this.
                        // it's ugly, but its efficient.
                        if let Some((gpumesh, verts, idxs)) =
                            MeshDescriptor::from_mesh(mesh, prim.clip_rect)
                        {
                            vertices.extend_from_slice(verts.as_slice());
                            indices.extend_from_slice(idxs.as_slice());

                            Some(gpumesh)
                        } else {
                            None
                        }
                    } else {
                        panic!("paint callbacks not supported")
                    }
                })
                .collect();

            self.last_vtx_capacity = vertices.len();
            self.last_idx_capacity = indices.len();

            self.buffers.update_vertex_buffer(dev, &vertices);
            self.buffers.update_index_buffer(dev, &indices);
        }

        // back up our state so we don't mess with the game and the game doesn't mess with us.
        // i actually had the idea to use BeginStateBlock and co. to "cache" the state we set every frame,
        // and just re-applying it everytime. just setting this manually takes around 50 microseconds on my machine.
        let _state = DxState::setup(dev, self.get_viewport());

        unsafe {
            expect!(
                dev.SetStreamSource(
                    0,
                    expect!(self.buffers.vtx.as_ref(), "unable to get vertex buffer"),
                    0,
                    std::mem::size_of::<GpuVertex>() as _
                ),
                "unable to set vertex stream source"
            );

            expect!(
                dev.SetIndices(expect!(
                    self.buffers.idx.as_ref(),
                    "unable to get index buffer"
                ),),
                "unable to set index buffer"
            );
        }

        let mut our_vtx_idx: usize = 0;
        let mut our_idx_idx: usize = 0;

        self.prims.iter().for_each(|mesh: &MeshDescriptor| unsafe {
            expect!(dev.SetScissorRect(&mesh.clip), "unable to set scissor rect");

            let texture = self.tex_man.get_by_id(mesh.texture_id);

            expect!(dev.SetTexture(0, texture), "unable to set texture");

            expect!(
                dev.DrawIndexedPrimitive(
                    D3DPT_TRIANGLELIST,
                    our_vtx_idx as _,
                    0,
                    mesh.vertices as _,
                    our_idx_idx as _,
                    (mesh.indices / 3usize) as _
                ),
                "unable to draw indexed prims"
            );

            our_vtx_idx += mesh.vertices;
            our_idx_idx += mesh.indices;
        });

        if !output.textures_delta.is_empty() {
            self.tex_man.process_free_deltas(&output.textures_delta);
        }

        for cmd in output.platform_output.commands {
            match cmd {
                egui::OutputCommand::CopyText(text) => {
                    let _ = WindowsClipboardContext.set_contents(text);
                }
                egui::OutputCommand::CopyImage(_) => {}
                egui::OutputCommand::OpenUrl(_) => {}
            }
        }
    }

    #[inline]
    pub fn wnd_proc(&mut self, umsg: u32, wparam: WPARAM, lparam: LPARAM) {
        // safe. we only write here, and only read elsewhere.
        self.input_man.process(umsg, wparam.0, lparam.0);
    }
}

impl<T: App> EguiDx9<T> {
    fn get_screen_size(&self) -> (f32, f32) {
        let mut rect = RECT::default();
        unsafe {
            expect!(
                GetClientRect(self.hwnd, &mut rect),
                "Failed to GetClientRect()"
            );
        }
        (
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        )
    }

    fn get_viewport(&self) -> D3DVIEWPORT9 {
        let (w, h) = self.get_screen_size();
        D3DVIEWPORT9 {
            X: 0,
            Y: 0,
            Width: w as _,
            Height: h as _,
            MinZ: 0.,
            MaxZ: 1.,
        }
    }
}

impl<T: App> EguiDx9<T> {
    pub fn state(&self) -> &T {
        &self.ui_state
    }

    pub fn state_mut(&mut self) -> &mut T {
        &mut self.ui_state
    }
}

impl<T: App> Drop for EguiDx9<T> {
    fn drop(&mut self) {
        self.buffers.delete_buffers();
        self.tex_man.deallocate_textures();
    }
}
