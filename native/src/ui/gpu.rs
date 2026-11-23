//! Draws egui's triangles with Direct3D 11.
//!
//! Direct3D 11 and nothing on top of it, because the layers on top are what cost: an OpenGL
//! context made the NVIDIA driver take 115 MB, wgpu (Direct3D 12) 158 MB, while a bare D3D11
//! device with its internal threading turned off is about 32 MB for the whole program. The
//! video card does the drawing, so a 4K window costs the processor no more than a small one.
//!
//! A frame is drawn only when it differs from the last one (see `Frame::fingerprint` in
//! ui/window.rs), so a still window costs neither the processor nor the video card anything.
//! The shaders are compiled ahead of time (shaders/egui.hlsl), so no shader compiler is
//! loaded at run time.

use std::collections::HashMap;

use egui::epaint::{ClippedPrimitive, ImageDelta, Primitive, TextureId};
use egui::TexturesDelta;
use windows::core::Interface;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

const VS: &[u8] = include_bytes!("shaders/egui_vs.cso");
const PS: &[u8] = include_bytes!("shaders/egui_ps.cso");
const BACKDROP_VS: &[u8] = include_bytes!("shaders/backdrop_vs.cso");
const BACKDROP_PS: &[u8] = include_bytes!("shaders/backdrop_ps.cso");

/// one textured draw of the last frame, kept so the frame can be drawn again over a moving
/// background without laying the interface out again
struct Call {
    texture: TextureId,
    rect: RECT,
    start: u32,
    count: u32,
}

/// the live background's pieces on the card (super::backdrop)
struct Backdrop {
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    params: ID3D11Buffer,
    /// the cover fading out and the one fading in
    from: Option<ID3D11ShaderResourceView>,
    to: Option<ID3D11ShaderResourceView>,
    /// drawn at half the window's size
    target: Option<(ID3D11RenderTargetView, ID3D11ShaderResourceView, (u32, u32))>,
    clock: super::backdrop::Clock,
}

struct GpuTexture {
    tex: ID3D11Texture2D,
    view: ID3D11ShaderResourceView,
    linear: bool,
    size: [usize; 2],
}

pub struct GpuRenderer {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    swap: IDXGISwapChain1,
    target: Option<ID3D11RenderTargetView>,
    size: (u32, u32),
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    layout: ID3D11InputLayout,
    blend: ID3D11BlendState,
    raster: ID3D11RasterizerState,
    linear: ID3D11SamplerState,
    nearest: ID3D11SamplerState,
    screen: ID3D11Buffer,
    vbuf: Option<(ID3D11Buffer, usize)>,
    ibuf: Option<(ID3D11Buffer, usize)>,
    textures: HashMap<TextureId, GpuTexture>,
    pub clear: [f32; 4],
    backdrop: Option<Backdrop>,
    /// the last frame's draws (see Call) and its scale
    calls: Vec<Call>,
    last_ppp: f32,
}

fn rgba(c: egui::Color32) -> [u8; 4] {
    c.to_array()
}

impl GpuRenderer {
    /// None when there is no usable video card (a remote desktop, a broken driver): the
    /// processor draws then (ui/soft.rs)
    pub fn new(hwnd: isize, width: u32, height: u32) -> Option<Self> {
        unsafe { Self::create(HWND(hwnd as _), width.max(1), height.max(1)).map_err(|e| eprintln!("[gpu] no Direct3D 11: {e}")).ok() }
    }

    unsafe fn create(hwnd: HWND, width: u32, height: u32) -> windows::core::Result<Self> {
        let mut device = None;
        let mut ctx = None;
        // one thread talks to the card, and the driver is asked not to start its own helper
        // threads for that: fewer threads, less memory, and nothing to gain for a player
        let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED | D3D11_CREATE_DEVICE_PREVENT_INTERNAL_THREADING_OPTIMIZATIONS;
        D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, Default::default(), flags, Some(&[D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_10_0]), D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut ctx))?;
        let device: ID3D11Device = device.ok_or_else(windows::core::Error::empty)?;
        let ctx: ID3D11DeviceContext = ctx.ok_or_else(windows::core::Error::empty)?;
        let dxgi: IDXGIDevice = device.cast()?;
        // one frame queued at most: what is drawn shows at the next refresh, not three later
        if let Ok(d1) = dxgi.cast::<IDXGIDevice1>() {
            let _ = d1.SetMaximumFrameLatency(1);
        }
        let factory: IDXGIFactory2 = dxgi.GetAdapter()?.GetParent()?;
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            ..Default::default()
        };
        let swap = factory.CreateSwapChainForHwnd(&device, hwnd, &desc, None, None)?;
        // Alt+Enter would make the player full screen: not wanted
        let _ = factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER);

        let mut vs = None;
        device.CreateVertexShader(VS, None, Some(&mut vs))?;
        let mut ps = None;
        device.CreatePixelShader(PS, None, Some(&mut ps))?;
        let elements = [
            D3D11_INPUT_ELEMENT_DESC { SemanticName: windows::core::s!("POSITION"), SemanticIndex: 0, Format: DXGI_FORMAT_R32G32_FLOAT, InputSlot: 0, AlignedByteOffset: 0, InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA, InstanceDataStepRate: 0 },
            D3D11_INPUT_ELEMENT_DESC { SemanticName: windows::core::s!("TEXCOORD"), SemanticIndex: 0, Format: DXGI_FORMAT_R32G32_FLOAT, InputSlot: 0, AlignedByteOffset: 8, InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA, InstanceDataStepRate: 0 },
            D3D11_INPUT_ELEMENT_DESC { SemanticName: windows::core::s!("COLOR"), SemanticIndex: 0, Format: DXGI_FORMAT_R8G8B8A8_UNORM, InputSlot: 0, AlignedByteOffset: 16, InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA, InstanceDataStepRate: 0 },
        ];
        let mut layout = None;
        device.CreateInputLayout(&elements, VS, Some(&mut layout))?;

        let mut blend_desc = D3D11_BLEND_DESC::default();
        blend_desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            // premultiplied colours
            SrcBlend: D3D11_BLEND_ONE,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_INV_DEST_ALPHA,
            DestBlendAlpha: D3D11_BLEND_ONE,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
        let mut blend = None;
        device.CreateBlendState(&blend_desc, Some(&mut blend))?;
        let raster_desc = D3D11_RASTERIZER_DESC { FillMode: D3D11_FILL_SOLID, CullMode: D3D11_CULL_NONE, ScissorEnable: true.into(), DepthClipEnable: false.into(), ..Default::default() };
        let mut raster = None;
        device.CreateRasterizerState(&raster_desc, Some(&mut raster))?;
        let sampler = |filter| -> windows::core::Result<ID3D11SamplerState> {
            let desc = D3D11_SAMPLER_DESC {
                Filter: filter,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MaxLOD: f32::MAX,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                ..Default::default()
            };
            let mut s = None;
            device.CreateSamplerState(&desc, Some(&mut s))?;
            s.ok_or_else(windows::core::Error::empty)
        };
        let linear = sampler(D3D11_FILTER_MIN_MAG_MIP_LINEAR)?;
        let nearest = sampler(D3D11_FILTER_MIN_MAG_MIP_POINT)?;
        let cb_desc = D3D11_BUFFER_DESC { ByteWidth: 16, Usage: D3D11_USAGE_DYNAMIC, BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32, CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32, ..Default::default() };
        let mut screen = None;
        device.CreateBuffer(&cb_desc, None, Some(&mut screen))?;

        // the live background: when its shaders do not load, the interface still draws
        let backdrop = (|| -> windows::core::Result<Backdrop> {
            let mut bvs = None;
            device.CreateVertexShader(BACKDROP_VS, None, Some(&mut bvs))?;
            let mut bps = None;
            device.CreatePixelShader(BACKDROP_PS, None, Some(&mut bps))?;
            let desc = D3D11_BUFFER_DESC { ByteWidth: 32, Usage: D3D11_USAGE_DYNAMIC, BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32, CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32, ..Default::default() };
            let mut params = None;
            device.CreateBuffer(&desc, None, Some(&mut params))?;
            let e = windows::core::Error::empty;
            Ok(Backdrop { vs: bvs.ok_or_else(e)?, ps: bps.ok_or_else(e)?, params: params.ok_or_else(e)?, from: None, to: None, target: None, clock: Default::default() })
        })()
        .map_err(|e| eprintln!("[gpu] no live background: {e}"))
        .ok();

        let none = windows::core::Error::empty;
        let mut me = Self {
            device,
            ctx,
            swap,
            target: None,
            size: (width, height),
            vs: vs.ok_or_else(none)?,
            ps: ps.ok_or_else(none)?,
            layout: layout.ok_or_else(none)?,
            blend: blend.ok_or_else(none)?,
            raster: raster.ok_or_else(none)?,
            linear,
            nearest,
            screen: screen.ok_or_else(none)?,
            vbuf: None,
            ibuf: None,
            textures: HashMap::new(),
            clear: [0.0, 0.0, 0.0, 1.0],
            backdrop,
            calls: Vec::new(),
            last_ppp: 1.0,
        };
        me.make_target()?;
        Ok(me)
    }

    unsafe fn make_target(&mut self) -> windows::core::Result<()> {
        let back: ID3D11Texture2D = self.swap.GetBuffer(0)?;
        let mut rtv = None;
        self.device.CreateRenderTargetView(&back, None, Some(&mut rtv))?;
        self.target = rtv;
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == self.size {
            return;
        }
        self.size = (width, height);
        unsafe {
            self.target = None;
            self.ctx.OMSetRenderTargets(None, None);
            self.ctx.Flush();
            if self.swap.ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0)).is_ok() {
                let _ = self.make_target();
            }
        }
    }

    pub fn set_textures(&mut self, delta: &TexturesDelta) {
        for (id, deltas) in &delta.set {
            for d in deltas {
                unsafe {
                    if let Err(e) = self.apply(*id, d) {
                        eprintln!("[gpu] texture: {e}");
                    }
                }
            }
        }
    }

    pub fn free_textures(&mut self, delta: &TexturesDelta) {
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    unsafe fn apply(&mut self, id: TextureId, d: &ImageDelta) -> windows::core::Result<()> {
        let egui::ImageData::Color(img) = &d.image;
        let [w, h] = img.size;
        if w == 0 || h == 0 {
            return Ok(());
        }
        let px: Vec<[u8; 4]> = img.pixels.iter().map(|c| rgba(*c)).collect();
        let linear = d.options.magnification == egui::TextureFilter::Linear;
        if let Some([x, y]) = d.pos {
            let Some(t) = self.textures.get(&id) else { return Ok(()) };
            if x + w > t.size[0] || y + h > t.size[1] {
                return Ok(());
            }
            let bx = D3D11_BOX { left: x as u32, top: y as u32, front: 0, right: (x + w) as u32, bottom: (y + h) as u32, back: 1 };
            self.ctx.UpdateSubresource(&t.tex, 0, Some(&bx), px.as_ptr() as _, (w * 4) as u32, 0);
            return Ok(());
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w as u32,
            Height: h as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA { pSysMem: px.as_ptr() as _, SysMemPitch: (w * 4) as u32, SysMemSlicePitch: 0 };
        let mut tex = None;
        self.device.CreateTexture2D(&desc, Some(&data), Some(&mut tex))?;
        let tex = tex.ok_or_else(windows::core::Error::empty)?;
        let mut view = None;
        self.device.CreateShaderResourceView(&tex, None, Some(&mut view))?;
        let view = view.ok_or_else(windows::core::Error::empty)?;
        self.textures.insert(id, GpuTexture { tex, view, linear, size: [w, h] });
        Ok(())
    }

    /// a dynamic buffer that holds at least `bytes`, made anew (half as big again) when it does not
    unsafe fn ensure(device: &ID3D11Device, slot: &mut Option<(ID3D11Buffer, usize)>, bytes: usize, bind: D3D11_BIND_FLAG) -> windows::core::Result<ID3D11Buffer> {
        if let Some((b, cap)) = slot {
            if *cap >= bytes {
                return Ok(b.clone());
            }
        }
        let cap = (bytes + bytes / 2).max(64 * 1024);
        let desc = D3D11_BUFFER_DESC { ByteWidth: cap as u32, Usage: D3D11_USAGE_DYNAMIC, BindFlags: bind.0 as u32, CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32, ..Default::default() };
        let mut b = None;
        device.CreateBuffer(&desc, None, Some(&mut b))?;
        let b = b.ok_or_else(windows::core::Error::empty)?;
        *slot = Some((b.clone(), cap));
        Ok(b)
    }

    /// draw a frame and show it
    pub fn render(&mut self, prims: &[ClippedPrimitive], ppp: f32) {
        unsafe {
            if let Err(e) = self.draw(prims, ppp) {
                eprintln!("[gpu] frame: {e}");
            }
        }
    }

    unsafe fn draw(&mut self, prims: &[ClippedPrimitive], ppp: f32) -> windows::core::Result<()> {
        self.upload(prims, ppp)?;
        self.replay()
    }

    /// the frame's triangles into the card's buffers, and the draws to make from them
    unsafe fn upload(&mut self, prims: &[ClippedPrimitive], ppp: f32) -> windows::core::Result<()> {
        self.calls.clear();
        self.last_ppp = ppp;
        // all the triangles of the frame go into one vertex and one index buffer
        let (mut nv, mut ni) = (0usize, 0usize);
        for p in prims {
            if let Primitive::Mesh(m) = &p.primitive {
                nv += m.vertices.len();
                ni += m.indices.len();
            }
        }
        if nv == 0 || ni == 0 {
            return Ok(());
        }
        let ctx = self.ctx.clone();
        let (w, h) = self.size;
        let vbuf = Self::ensure(&self.device, &mut self.vbuf, nv * 20, D3D11_BIND_VERTEX_BUFFER)?;
        let ibuf = Self::ensure(&self.device, &mut self.ibuf, ni * 4, D3D11_BIND_INDEX_BUFFER)?;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        ctx.Map(&vbuf, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
        let mut vptr = mapped.pData as *mut u8;
        let mut mi = D3D11_MAPPED_SUBRESOURCE::default();
        if let Err(e) = ctx.Map(&ibuf, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mi)) {
            ctx.Unmap(&vbuf, 0);
            return Err(e);
        }
        let mut iptr = mi.pData as *mut u32;
        let (mut vbase, mut ibase) = (0u32, 0u32);
        for p in prims {
            let Primitive::Mesh(m) = &p.primitive else { continue };
            for v in &m.vertices {
                let rec: [f32; 4] = [v.pos.x, v.pos.y, v.uv.x, v.uv.y];
                std::ptr::copy_nonoverlapping(rec.as_ptr() as *const u8, vptr, 16);
                std::ptr::copy_nonoverlapping(rgba(v.color).as_ptr(), vptr.add(16), 4);
                vptr = vptr.add(20);
            }
            for &i in &m.indices {
                *iptr = i + vbase;
                iptr = iptr.add(1);
            }
            let c = p.clip_rect;
            let rect = RECT {
                left: ((c.min.x * ppp).floor() as i32).clamp(0, w as i32),
                top: ((c.min.y * ppp).floor() as i32).clamp(0, h as i32),
                right: ((c.max.x * ppp).ceil() as i32).clamp(0, w as i32),
                bottom: ((c.max.y * ppp).ceil() as i32).clamp(0, h as i32),
            };
            if rect.left < rect.right && rect.top < rect.bottom {
                self.calls.push(Call { texture: m.texture_id, rect, start: ibase, count: m.indices.len() as u32 });
            }
            vbase += m.vertices.len() as u32;
            ibase += m.indices.len() as u32;
        }
        ctx.Unmap(&ibuf, 0);
        ctx.Unmap(&vbuf, 0);
        let screen = [w as f32 / ppp, h as f32 / ppp, 0.0, 0.0];
        let mut cb = D3D11_MAPPED_SUBRESOURCE::default();
        ctx.Map(&self.screen, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut cb))?;
        std::ptr::copy_nonoverlapping(screen.as_ptr(), cb.pData as *mut f32, 4);
        ctx.Unmap(&self.screen, 0);
        Ok(())
    }

    /// the frame uses the live background
    pub fn backdrop_used(&self) -> bool {
        self.backdrop.is_some() && self.calls.iter().any(|c| c.texture == super::backdrop::TEXTURE)
    }

    /// a new cover is still fading in
    pub fn backdrop_fading(&self) -> bool {
        self.backdrop.as_ref().is_some_and(|b| b.clock.fading())
    }

    /// the window was out of sight: the background's clock does not jump when it comes back
    pub fn backdrop_pause(&mut self) {
        if let Some(b) = self.backdrop.as_mut() {
            b.clock.pause();
        }
    }

    /// the background moves on: drawn again under the last frame's interface, which is not
    /// laid out again (its triangles are still in the card's buffers)
    pub fn animate(&mut self) {
        unsafe {
            if let Err(e) = self.replay() {
                eprintln!("[gpu] frame: {e}");
            }
        }
    }

    unsafe fn cover_texture(&self, px: &[u8]) -> windows::core::Result<ID3D11ShaderResourceView> {
        let n = super::backdrop::TEX_SIZE;
        let desc = D3D11_TEXTURE2D_DESC { Width: n, Height: n, MipLevels: 1, ArraySize: 1, Format: DXGI_FORMAT_R8G8B8A8_UNORM, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Usage: D3D11_USAGE_IMMUTABLE, BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32, ..Default::default() };
        let data = D3D11_SUBRESOURCE_DATA { pSysMem: px.as_ptr() as _, SysMemPitch: n * 4, SysMemSlicePitch: 0 };
        let mut tex = None;
        self.device.CreateTexture2D(&desc, Some(&data), Some(&mut tex))?;
        let tex = tex.ok_or_else(windows::core::Error::empty)?;
        let mut view = None;
        self.device.CreateShaderResourceView(&tex, None, Some(&mut view))?;
        view.ok_or_else(windows::core::Error::empty)
    }

    /// the background into its own texture at half the window's size
    unsafe fn draw_backdrop(&mut self) -> windows::core::Result<()> {
        let (w, h) = ((self.size.0 / 2).max(2), (self.size.1 / 2).max(2));
        // a cover that arrived fades in over the last one
        if let Some(px) = super::backdrop::take_ready() {
            let view = self.cover_texture(&px)?;
            if let Some(b) = self.backdrop.as_mut() {
                b.from = b.to.take().or_else(|| Some(view.clone()));
                b.to = Some(view);
                b.clock.fade();
            }
        }
        if self.backdrop.as_ref().is_some_and(|b| b.to.is_none()) {
            let view = self.cover_texture(&super::backdrop::fallback())?;
            if let Some(b) = self.backdrop.as_mut() {
                b.from = Some(view.clone());
                b.to = Some(view);
            }
        }
        if self.backdrop.as_ref().is_some_and(|b| b.target.as_ref().is_none_or(|t| t.2 != (w, h))) {
            let desc = D3D11_TEXTURE2D_DESC { Width: w, Height: h, MipLevels: 1, ArraySize: 1, Format: DXGI_FORMAT_R8G8B8A8_UNORM, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Usage: D3D11_USAGE_DEFAULT, BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32, ..Default::default() };
            let mut tex = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut tex))?;
            let tex = tex.ok_or_else(windows::core::Error::empty)?;
            let (mut rtv, mut srv) = (None, None);
            self.device.CreateRenderTargetView(&tex, None, Some(&mut rtv))?;
            self.device.CreateShaderResourceView(&tex, None, Some(&mut srv))?;
            if let (Some(b), Some(rtv), Some(srv)) = (self.backdrop.as_mut(), rtv, srv) {
                b.target = Some((rtv, srv, (w, h)));
            }
        }
        let (playing, animated) = super::backdrop::mode();
        let ctx = self.ctx.clone();
        let Some(b) = self.backdrop.as_mut() else { return Ok(()) };
        let Some((rtv, _, _)) = b.target.clone() else { return Ok(()) };
        let (mix, time, dim) = b.clock.tick(playing, animated);
        let params: [f32; 8] = [mix, time, 0.55, 0.0, dim, w as f32, h as f32, 0.0];
        let mut cb = D3D11_MAPPED_SUBRESOURCE::default();
        ctx.Map(&b.params, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut cb))?;
        std::ptr::copy_nonoverlapping(params.as_ptr(), cb.pData as *mut f32, 8);
        ctx.Unmap(&b.params, 0);
        ctx.OMSetRenderTargets(Some(&[Some(rtv)]), None);
        ctx.OMSetBlendState(None, None, 0xffff_ffff);
        ctx.RSSetState(&self.raster);
        ctx.RSSetViewports(Some(&[D3D11_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: w as f32, Height: h as f32, MinDepth: 0.0, MaxDepth: 1.0 }]));
        ctx.RSSetScissorRects(Some(&[RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }]));
        ctx.IASetInputLayout(None);
        ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        ctx.VSSetShader(&b.vs, None);
        ctx.PSSetShader(&b.ps, None);
        ctx.PSSetConstantBuffers(0, Some(&[Some(b.params.clone())]));
        ctx.PSSetShaderResources(0, Some(&[b.from.clone(), b.to.clone()]));
        ctx.PSSetSamplers(0, Some(&[Some(self.linear.clone())]));
        ctx.Draw(3, 0);
        // the texture is read by the interface next: unbound as a target first
        ctx.OMSetRenderTargets(None, None);
        ctx.PSSetShaderResources(0, Some(&[None, None]));
        Ok(())
    }

    /// the last uploaded frame on screen (with the background drawn first when it is used)
    unsafe fn replay(&mut self) -> windows::core::Result<()> {
        let Some(target) = self.target.clone() else { return Ok(()) };
        if self.backdrop_used() {
            self.draw_backdrop()?;
        }
        let ctx = self.ctx.clone();
        ctx.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
        ctx.ClearRenderTargetView(&target, &self.clear);
        let (w, h) = self.size;
        if let (false, Some((vbuf, _)), Some((ibuf, _))) = (self.calls.is_empty(), self.vbuf.clone(), self.ibuf.clone()) {
            ctx.RSSetViewports(Some(&[D3D11_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: w as f32, Height: h as f32, MinDepth: 0.0, MaxDepth: 1.0 }]));
            ctx.RSSetState(&self.raster);
            ctx.OMSetBlendState(&self.blend, Some(&[0.0; 4]), 0xffff_ffff);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.IASetInputLayout(&self.layout);
            let stride = 20u32;
            let offset = 0u32;
            ctx.IASetVertexBuffers(0, 1, Some(&Some(vbuf)), Some(&stride), Some(&offset));
            ctx.IASetIndexBuffer(&ibuf, DXGI_FORMAT_R32_UINT, 0);
            ctx.VSSetShader(&self.vs, None);
            ctx.VSSetConstantBuffers(0, Some(&[Some(self.screen.clone())]));
            ctx.PSSetShader(&self.ps, None);
            let backdrop_view = self.backdrop.as_ref().and_then(|b| b.target.as_ref().map(|t| t.1.clone()));
            for c in &self.calls {
                let (view, linear) = if c.texture == super::backdrop::TEXTURE {
                    match &backdrop_view {
                        Some(v) => (v.clone(), true),
                        None => continue,
                    }
                } else {
                    match self.textures.get(&c.texture) {
                        Some(t) => (t.view.clone(), t.linear),
                        None => continue,
                    }
                };
                ctx.RSSetScissorRects(Some(&[c.rect]));
                ctx.PSSetShaderResources(0, Some(&[Some(view)]));
                ctx.PSSetSamplers(0, Some(&[Some(if linear { self.linear.clone() } else { self.nearest.clone() })]));
                ctx.DrawIndexed(c.count, c.start, 0);
            }
        }
        // shown at the next refresh of the screen; the frame waits for it, so a busy window
        // never draws faster than the screen can show
        let _ = self.swap.Present(1, DXGI_PRESENT(0));
        Ok(())
    }
}
