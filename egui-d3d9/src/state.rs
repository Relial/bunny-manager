use anyhow::{anyhow, Context as _, Result};
use windows::Win32::Graphics::Direct3D9::{
    IDirect3DDevice9, IDirect3DStateBlock9, D3DBLENDOP_ADD, D3DBLEND_INVSRCALPHA, D3DBLEND_ONE,
    D3DCULL_NONE, D3DFILL_SOLID, D3DRS_ALPHABLENDENABLE, D3DRS_ALPHATESTENABLE, D3DRS_BLENDOP,
    D3DRS_BLENDOPALPHA, D3DRS_CLIPPING, D3DRS_COLORWRITEENABLE, D3DRS_CULLMODE, D3DRS_DESTBLEND,
    D3DRS_DESTBLENDALPHA, D3DRS_FILLMODE, D3DRS_FOGENABLE, D3DRS_LASTPIXEL, D3DRS_LIGHTING,
    D3DRS_RANGEFOGENABLE, D3DRS_SCISSORTESTENABLE, D3DRS_SEPARATEALPHABLENDENABLE, D3DRS_SHADEMODE,
    D3DRS_SPECULARENABLE, D3DRS_SRCBLEND, D3DRS_SRCBLENDALPHA, D3DRS_SRGBWRITEENABLE,
    D3DRS_STENCILENABLE, D3DRS_TEXTUREFACTOR, D3DRS_ZENABLE, D3DRS_ZWRITEENABLE, D3DSAMP_ADDRESSU,
    D3DSAMP_ADDRESSV, D3DSAMP_ADDRESSW, D3DSAMP_BORDERCOLOR, D3DSAMP_MAGFILTER, D3DSAMP_MINFILTER,
    D3DSAMP_MIPFILTER, D3DSBT_ALL, D3DSHADE_GOURAUD, D3DTADDRESS_CLAMP, D3DTA_CURRENT,
    D3DTA_DIFFUSE, D3DTA_TEXTURE, D3DTEXF_LINEAR, D3DTOP_DISABLE, D3DTOP_MODULATE,
    D3DTRANSFORMSTATETYPE, D3DTSS_ALPHAARG0, D3DTSS_ALPHAARG1, D3DTSS_ALPHAARG2, D3DTSS_ALPHAOP,
    D3DTSS_COLORARG0, D3DTSS_COLORARG1, D3DTSS_COLORARG2, D3DTSS_COLOROP, D3DTS_PROJECTION,
    D3DTS_VIEW, D3DVIEWPORT9,
};
use windows_numerics::Matrix4x4;

use crate::mesh::FVF_CUSTOMVERTEX;

#[derive(Default, Debug)]
pub struct GpuState {
    state: Option<IDirect3DStateBlock9>,
    game_state: Option<IDirect3DStateBlock9>,
}

impl GpuState {
    pub fn backup(&mut self, device: &IDirect3DDevice9) -> Result<()> {
        unsafe {
            if let Some(game_state) = &self.game_state {
                game_state
                    .Capture()
                    .context("Failed to capture state block")?;
            } else {
                let state = device
                    .CreateStateBlock(D3DSBT_ALL)
                    .context("Failed to create game state block")?;
                state.Capture().context("Failed to capture state block")?;
                self.game_state = Some(state);
            }
        }
        Ok(())
    }

    pub fn setup(&mut self, device: &IDirect3DDevice9, viewport: D3DVIEWPORT9) -> Result<()> {
        setup_nonblock_state(device, viewport).context("Failed to setup non-block state")?;
        unsafe {
            if let Some(new_state) = &self.state {
                new_state.Apply().context("Failed to apply saved state")?;
            } else {
                self.state = Some(setup_state_block(device)?);
            }
        }
        Ok(())
    }

    pub fn restore(&mut self) -> Result<()> {
        let saved_state = self
            .game_state
            .as_ref()
            .ok_or(anyhow!("No game state block saved"))?;
        unsafe {
            saved_state
                .Apply()
                .context("Failed to apply saved game state")?;
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.state = None;
        self.game_state = None;
    }
}

fn setup_nonblock_state(device: &IDirect3DDevice9, viewport: D3DVIEWPORT9) -> Result<()> {
    unsafe {
        device
            .SetViewport(&viewport)
            .context("Failed to set viewport")?;

        let l = 0.5;
        let r = viewport.Width as f32 + 0.5;
        let t = 0.5;
        let b = viewport.Height as f32 + 0.5;
        let mat_proj = Matrix4x4 {
            M11: 2.0 / (r - l),
            M12: 0.0,
            M13: 0.0,
            M14: 0.0,
            M21: 0.0,
            M22: 2.0 / (t - b),
            M23: 0.0,
            M24: 0.0,
            M31: 0.0,
            M32: 0.0,
            M33: 0.5,
            M34: 0.0,
            M41: (l + r) / (l - r),
            M42: (t + b) / (b - t),
            M43: 0.5,
            M44: 1.0,
        };
        device
            .SetTransform(D3DTS_PROJECTION, &mat_proj)
            .context("Failed to set projection matrix")?;
        Ok(())
    }
}

fn setup_state_block(device: &IDirect3DDevice9) -> Result<IDirect3DStateBlock9> {
    unsafe {
        device
            .BeginStateBlock()
            .context("Failed to begin state block")?;

        // set up fvf
        device
            .SetPixelShader(None)
            .context("Failed to set pixel shader")?;
        device
            .SetVertexShader(None)
            .context("Failed to set vertex shader")?;
        device.SetFVF(FVF_CUSTOMVERTEX).context("Failed SetFVF")?;

        // set up matrix
        let mat_ident = Matrix4x4 {
            M11: 1.0,
            M22: 1.0,
            M33: 1.0,
            M44: 1.0,
            ..Default::default()
        };

        device
            .SetTransform(D3DTRANSFORMSTATETYPE(256), &mat_ident)
            .context("Failed to set world matrix")?;
        device
            .SetTransform(D3DTS_VIEW, &mat_ident)
            .context("Failed to set view matrix")?;

        // set up render state
        device.SetRenderState(D3DRS_FILLMODE, D3DFILL_SOLID.0 as _)?;
        device.SetRenderState(D3DRS_SHADEMODE, D3DSHADE_GOURAUD.0 as _)?;
        device.SetRenderState(D3DRS_ZENABLE, false as _)?;
        device.SetRenderState(D3DRS_ZWRITEENABLE, false as _)?;
        device.SetRenderState(D3DRS_ALPHATESTENABLE, false as _)?;
        device.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE.0 as _)?;
        device.SetRenderState(D3DRS_ALPHABLENDENABLE, true as _)?;
        device.SetRenderState(D3DRS_BLENDOP, D3DBLENDOP_ADD.0 as _)?;
        device.SetRenderState(D3DRS_SRCBLEND, D3DBLEND_ONE.0 as _)?;
        device.SetRenderState(D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as _)?;
        device.SetRenderState(D3DRS_SEPARATEALPHABLENDENABLE, true as _)?;
        device.SetRenderState(D3DRS_BLENDOPALPHA, D3DBLENDOP_ADD.0 as _)?;
        device.SetRenderState(D3DRS_SRCBLENDALPHA, D3DBLEND_ONE.0 as _)?;
        device.SetRenderState(D3DRS_DESTBLENDALPHA, D3DBLEND_INVSRCALPHA.0 as _)?;
        device.SetRenderState(D3DRS_SCISSORTESTENABLE, true as _)?;
        device.SetRenderState(D3DRS_FOGENABLE, false as _)?;
        device.SetRenderState(D3DRS_RANGEFOGENABLE, false as _)?;
        device.SetRenderState(D3DRS_SPECULARENABLE, false as _)?;
        device.SetRenderState(D3DRS_STENCILENABLE, false as _)?;
        device.SetRenderState(D3DRS_CLIPPING, true as _)?;
        device.SetRenderState(D3DRS_LIGHTING, false as _)?;
        device.SetRenderState(D3DRS_TEXTUREFACTOR, 0xFFFFFFFF)?;
        device.SetRenderState(D3DRS_COLORWRITEENABLE, 0xFFFFFFFF)?;
        device.SetRenderState(D3DRS_SRGBWRITEENABLE, false as _)?;
        device.SetRenderState(D3DRS_LASTPIXEL, true as _)?;

        // set up texture stages
        device.SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_MODULATE.0 as _)?;
        device.SetTextureStageState(0, D3DTSS_COLORARG0, D3DTA_CURRENT)?;
        device.SetTextureStageState(0, D3DTSS_COLORARG1, D3DTA_TEXTURE)?;
        device.SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE)?;
        device.SetTextureStageState(0, D3DTSS_ALPHAOP, D3DTOP_MODULATE.0 as _)?;
        device.SetTextureStageState(0, D3DTSS_ALPHAARG0, D3DTA_CURRENT)?;
        device.SetTextureStageState(0, D3DTSS_ALPHAARG1, D3DTA_TEXTURE)?;
        device.SetTextureStageState(0, D3DTSS_ALPHAARG2, D3DTA_DIFFUSE)?;

        device.SetTextureStageState(1, D3DTSS_COLOROP, D3DTOP_DISABLE.0 as _)?;
        device.SetTextureStageState(1, D3DTSS_ALPHAOP, D3DTOP_DISABLE.0 as _)?;

        // set up sampler
        device.SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as _)?;
        device.SetSamplerState(0, D3DSAMP_MIPFILTER, D3DTEXF_LINEAR.0 as _)?;
        device.SetSamplerState(0, D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as _)?;
        device.SetSamplerState(0, D3DSAMP_BORDERCOLOR, 0xFFFFFFFF)?;
        device.SetSamplerState(0, D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as _)?;
        device.SetSamplerState(0, D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as _)?;
        device.SetSamplerState(0, D3DSAMP_ADDRESSW, D3DTADDRESS_CLAMP.0 as _)?;

        device
            .EndStateBlock()
            .context("Failed to finish state block")
    }
}
