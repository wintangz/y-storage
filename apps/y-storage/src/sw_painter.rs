use std::collections::HashMap;
use std::ptr::null_mut;
use std::slice;

use egui::epaint::{ImageData, Primitive, TextureId, WHITE_UV};
use egui::{ClippedPrimitive, Color32, TexturesDelta};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC, HGDIOBJ, SRCCOPY, SelectObject,
};

struct CpuTexture {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>, // Premultiplied sRGBA [R, G, B, A]
}

struct DibBackBuffer {
    cap_w: usize,
    cap_h: usize,
    mem_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    bits: *mut u32,
}

impl DibBackBuffer {
    fn new(hdc_ref: HDC, cap_w: usize, cap_h: usize) -> Option<Self> {
        if cap_w == 0 || cap_h == 0 || hdc_ref.is_null() {
            return None;
        }
        let mem_dc = unsafe { CreateCompatibleDC(hdc_ref) };
        if mem_dc.is_null() {
            return None;
        }
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: cap_w as i32,
                biHeight: -(cap_h as i32), // Top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [unsafe { std::mem::zeroed() }],
        };
        let mut raw_bits: *mut std::ffi::c_void = null_mut();
        let bitmap = unsafe {
            CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut raw_bits, null_mut(), 0)
        };
        if bitmap.is_null() || raw_bits.is_null() {
            unsafe {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                DeleteDC(mem_dc);
            }
            return None;
        }
        let old_bitmap = unsafe { SelectObject(mem_dc, bitmap) };
        Some(Self {
            cap_w,
            cap_h,
            mem_dc,
            bitmap,
            old_bitmap,
            bits: raw_bits.cast::<u32>(),
        })
    }
}

impl Drop for DibBackBuffer {
    fn drop(&mut self) {
        unsafe {
            if !self.mem_dc.is_null() {
                if !self.old_bitmap.is_null() {
                    SelectObject(self.mem_dc, self.old_bitmap);
                }
                if !self.bitmap.is_null() {
                    DeleteObject(self.bitmap);
                }
                DeleteDC(self.mem_dc);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PaintStats {
    pub primitives: usize,
    pub triangles_total: usize,
    pub solid_quads: usize,
    pub solid_tri_opaque: usize,
    pub solid_tri_alpha: usize,
    pub grad_feather_tri: usize,
    pub textured_tri: usize,
    pub clear_ms: f64,
    pub quad_ms: f64,
    pub solid_tri_ms: f64,
    pub grad_tri_ms: f64,
    pub tex_tri_ms: f64,
    pub bitblt_ms: f64,
}

pub struct SoftwarePainter {
    width: usize,
    height: usize,
    dib: Option<DibBackBuffer>,
    fallback_fb: Vec<u32>,
    textures: HashMap<TextureId, CpuTexture>,
}

impl SoftwarePainter {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            dib: None,
            fallback_fb: Vec::new(),
            textures: HashMap::new(),
        }
    }

    pub fn has_ready_backbuffer(&self, width: usize, height: usize) -> bool {
        self.width == width
            && self.height == height
            && (self
                .dib
                .as_ref()
                .is_some_and(|d| width <= d.cap_w && height <= d.cap_h)
                || self.fallback_fb.len() == width * height)
    }

    pub fn apply_textures(&mut self, delta: &TexturesDelta) {
        for (id, image_delta) in &delta.set {
            let (w, h, pixels) = decode_image(&image_delta.image);
            if let Some(pos) = image_delta.pos {
                if let Some(tex) = self.textures.get_mut(id) {
                    for row in 0..h {
                        let dy = pos[1] + row;
                        if dy >= tex.height {
                            break;
                        }
                        for col in 0..w {
                            let dx = pos[0] + col;
                            if dx >= tex.width {
                                break;
                            }
                            tex.pixels[dy * tex.width + dx] = pixels[row * w + col];
                        }
                    }
                    continue;
                }
            }
            self.textures.insert(
                *id,
                CpuTexture {
                    width: w,
                    height: h,
                    pixels,
                },
            );
        }
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    fn ensure_backbuffer(&mut self, hdc_ref: HDC, width: usize, height: usize) -> usize {
        self.width = width;
        self.height = height;
        if hdc_ref.is_null() {
            self.dib = None;
            self.fallback_fb.resize(width * height, 0);
            return width;
        }
        let need_alloc = match &self.dib {
            Some(d) => width > d.cap_w || height > d.cap_h,
            None => true,
        };
        if need_alloc {
            // Pool capacity in 256px chunks so live window-edge resize dragging avoids per-frame GDI allocations.
            let prev_w = self.dib.as_ref().map_or(0, |d| d.cap_w);
            let prev_h = self.dib.as_ref().map_or(0, |d| d.cap_h);
            let cap_w = ((width.max(prev_w) + 255) & !255).max(256);
            let cap_h = ((height.max(prev_h) + 255) & !255).max(256);
            self.dib = DibBackBuffer::new(hdc_ref, cap_w, cap_h);
        }
        if let Some(d) = &self.dib {
            self.fallback_fb = Vec::new();
            d.cap_w
        } else {
            self.fallback_fb.resize(width * height, 0);
            width
        }
    }

    pub fn backbuffer_pixels(&self) -> &[u32] {
        let len = self.width * self.height;
        &self.fallback_fb[..len]
    }

    pub fn present_rect(&self, hdc: HDC, dirty: Option<RECT>) {
        if hdc.is_null() || self.width == 0 || self.height == 0 {
            return;
        }
        let (rx, ry, rw, rh) = match dirty {
            Some(r) => {
                let x0 = r.left.clamp(0, self.width as i32);
                let y0 = r.top.clamp(0, self.height as i32);
                let x1 = r.right.clamp(0, self.width as i32);
                let y1 = r.bottom.clamp(0, self.height as i32);
                if x1 <= x0 || y1 <= y0 {
                    (0, 0, self.width as i32, self.height as i32)
                } else {
                    (x0, y0, x1 - x0, y1 - y0)
                }
            }
            None => (0, 0, self.width as i32, self.height as i32),
        };
        if let Some(d) = &self.dib {
            unsafe {
                BitBlt(hdc, rx, ry, rw, rh, d.mem_dc, rx, ry, SRCCOPY);
            }
        }
    }

    pub fn paint_and_blit(
        &mut self,
        hdc: HDC,
        width: usize,
        height: usize,
        pixels_per_point: f32,
        primitives: &[ClippedPrimitive],
        clear_rgb: u32,
        present_clip: Option<RECT>,
    ) {
        self.paint_and_blit_profiled(
            hdc,
            width,
            height,
            pixels_per_point,
            primitives,
            clear_rgb,
            present_clip,
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paint_and_blit_profiled(
        &mut self,
        hdc: HDC,
        width: usize,
        height: usize,
        pixels_per_point: f32,
        primitives: &[ClippedPrimitive],
        clear_rgb: u32,
        present_clip: Option<RECT>,
        mut stats: Option<&mut PaintStats>,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let stride = self.ensure_backbuffer(hdc, width, height);
        let t_clr = stats.as_ref().map(|_| std::time::Instant::now());
        {
            let fb: &mut [u32] = if let Some(d) = &self.dib {
                unsafe { slice::from_raw_parts_mut(d.bits, d.cap_w * height) }
            } else {
                &mut self.fallback_fb[..width * height]
            };
            if stride == width {
                fb.fill(clear_rgb);
            } else {
                for py in 0..height {
                    let row = py * stride;
                    fb[row..row + width].fill(clear_rgb);
                }
            }
        }
        if let (Some(st), Some(t0)) = (stats.as_deref_mut(), t_clr) {
            st.clear_ms += t0.elapsed().as_secs_f64() * 1000.0;
        }

        let fb: &mut [u32] = if let Some(d) = &self.dib {
            unsafe { slice::from_raw_parts_mut(d.bits, d.cap_w * height) }
        } else {
            &mut self.fallback_fb[..width * height]
        };

        for cp in primitives {
            let cmin_x = (cp.clip_rect.min.x * pixels_per_point)
                .floor()
                .clamp(0.0, width as f32) as usize;
            let cmin_y = (cp.clip_rect.min.y * pixels_per_point)
                .floor()
                .clamp(0.0, height as f32) as usize;
            let cmax_x = (cp.clip_rect.max.x * pixels_per_point)
                .ceil()
                .clamp(0.0, width as f32) as usize;
            let cmax_y = (cp.clip_rect.max.y * pixels_per_point)
                .ceil()
                .clamp(0.0, height as f32) as usize;
            if cmin_x >= cmax_x || cmin_y >= cmax_y {
                continue;
            }

            if let Primitive::Mesh(mesh) = &cp.primitive {
                if let Some(st) = stats.as_deref_mut() {
                    st.primitives += 1;
                    st.triangles_total += mesh.indices.len() / 3;
                }
                let tex = self.textures.get(&mesh.texture_id);
                let mut i = 0usize;
                while i + 3 <= mesh.indices.len() {
                    if i + 6 <= mesh.indices.len() {
                        let t0 = &mesh.indices[i..i + 6];
                        let v0_untex = mesh.vertices[t0[0] as usize].uv == WHITE_UV;
                        if v0_untex {
                            if let Some((qx0, qy0, qx1, qy1, col)) =
                                detect_solid_quad(&mesh.vertices, t0, pixels_per_point)
                            {
                                let t_q = stats.as_ref().map(|_| std::time::Instant::now());
                                fill_solid_rect(
                                    fb, stride, cmin_x, cmin_y, cmax_x, cmax_y, qx0, qy0, qx1, qy1, col,
                                );
                                if let (Some(st), Some(t0)) = (stats.as_deref_mut(), t_q) {
                                    st.solid_quads += 1;
                                    st.quad_ms += t0.elapsed().as_secs_f64() * 1000.0;
                                }
                                i += 6;
                                continue;
                            }
                        } else if let Some(tq) = tex.and_then(|t| {
                            detect_aligned_tex_quad(&mesh.vertices, t0, pixels_per_point, t)
                        }) {
                            let t_tex = stats.as_ref().map(|_| std::time::Instant::now());
                            fill_aligned_tex_rect(
                                fb, stride, cmin_x, cmin_y, cmax_x, cmax_y, &tq, tex.unwrap(),
                            );
                            if let (Some(st), Some(t0)) = (stats.as_deref_mut(), t_tex) {
                                st.textured_tri += 2;
                                st.tex_tri_ms += t0.elapsed().as_secs_f64() * 1000.0;
                            }
                            i += 6;
                            continue;
                        }
                    }

                    let tri = &mesh.indices[i..i + 3];
                    let v0 = &mesh.vertices[tri[0] as usize];
                    let v1 = &mesh.vertices[tri[1] as usize];
                    let v2 = &mesh.vertices[tri[2] as usize];
                    let t_tri = stats.as_ref().map(|_| std::time::Instant::now());
                    rasterize_triangle(
                        fb,
                        stride,
                        cmin_x,
                        cmin_y,
                        cmax_x,
                        cmax_y,
                        pixels_per_point,
                        v0,
                        v1,
                        v2,
                        tex,
                    );
                    if let (Some(st), Some(t0)) = (stats.as_deref_mut(), t_tri) {
                        let dt = t0.elapsed().as_secs_f64() * 1000.0;
                        let c0 = v0.color.to_array();
                        let c1 = v1.color.to_array();
                        let c2 = v2.color.to_array();
                        let untex = v0.uv == WHITE_UV && v1.uv == WHITE_UV && v2.uv == WHITE_UV;
                        if !untex {
                            st.textured_tri += 1;
                            st.tex_tri_ms += dt;
                        } else if c0 == c1 && c1 == c2 {
                            if c0[3] == 255 {
                                st.solid_tri_opaque += 1;
                            } else {
                                st.solid_tri_alpha += 1;
                            }
                            st.solid_tri_ms += dt;
                        } else {
                            st.grad_feather_tri += 1;
                            st.grad_tri_ms += dt;
                        }
                    }
                    i += 3;
                }
            }
        }

        if !hdc.is_null() {
            let t_blit = stats.as_ref().map(|_| std::time::Instant::now());
            self.present_rect(hdc, present_clip);
            if let (Some(st), Some(t0)) = (stats.as_deref_mut(), t_blit) {
                st.bitblt_ms += t0.elapsed().as_secs_f64() * 1000.0;
            }
        }
    }
}

struct AlignedTexQuad {
    x0_f: f32,
    y0_f: f32,
    x1_f: f32,
    y1_f: f32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    col: [u8; 4],
}

#[inline]
fn detect_aligned_tex_quad(
    verts: &[egui::epaint::Vertex],
    idx6: &[u32],
    ppp: f32,
    _tex: &CpuTexture,
) -> Option<AlignedTexQuad> {
    let v0 = &verts[idx6[0] as usize];
    if v0.uv == WHITE_UV {
        return None;
    }
    let c = v0.color.to_array();
    if c[3] == 0 {
        return None;
    }
    let mut xs = [f32::INFINITY, f32::NEG_INFINITY];
    let mut ys = [f32::INFINITY, f32::NEG_INFINITY];
    for &idx in idx6 {
        let v = &verts[idx as usize];
        if v.uv == WHITE_UV || v.color.to_array() != c {
            return None;
        }
        xs[0] = xs[0].min(v.pos.x);
        xs[1] = xs[1].max(v.pos.x);
        ys[0] = ys[0].min(v.pos.y);
        ys[1] = ys[1].max(v.pos.y);
    }
    if xs[1] <= xs[0] + 1e-3 || ys[1] <= ys[0] + 1e-3 {
        return None;
    }
    let mut u_left: Option<f32> = None;
    let mut u_right: Option<f32> = None;
    let mut v_top: Option<f32> = None;
    let mut v_bot: Option<f32> = None;
    for &idx in idx6 {
        let v = &verts[idx as usize];
        let at_l = (v.pos.x - xs[0]).abs() < 1e-3;
        let at_r = (v.pos.x - xs[1]).abs() < 1e-3;
        let at_t = (v.pos.y - ys[0]).abs() < 1e-3;
        let at_b = (v.pos.y - ys[1]).abs() < 1e-3;
        if !(at_l || at_r) || !(at_t || at_b) {
            return None;
        }
        if at_l {
            if let Some(ul) = u_left {
                if (ul - v.uv.x).abs() > 1e-5 {
                    return None;
                }
            } else {
                u_left = Some(v.uv.x);
            }
        } else if let Some(ur) = u_right {
            if (ur - v.uv.x).abs() > 1e-5 {
                return None;
            }
        } else {
            u_right = Some(v.uv.x);
        }
        if at_t {
            if let Some(vt) = v_top {
                if (vt - v.uv.y).abs() > 1e-5 {
                    return None;
                }
            } else {
                v_top = Some(v.uv.y);
            }
        } else if let Some(vb) = v_bot {
            if (vb - v.uv.y).abs() > 1e-5 {
                return None;
            }
        } else {
            v_bot = Some(v.uv.y);
        }
    }
    let (Some(u0), Some(u1), Some(v0_uv), Some(v1_uv)) = (u_left, u_right, v_top, v_bot) else {
        return None;
    };
    Some(AlignedTexQuad {
        x0_f: xs[0] * ppp,
        y0_f: ys[0] * ppp,
        x1_f: xs[1] * ppp,
        y1_f: ys[1] * ppp,
        u0,
        v0: v0_uv,
        u1,
        v1: v1_uv,
        col: c,
    })
}

#[inline(always)]
fn div255(x: u32) -> u32 {
    let t = x + 128;
    (t + (t >> 8)) >> 8
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn fill_aligned_tex_rect(
    fb: &mut [u32],
    fb_stride: usize,
    clip_min_x: usize,
    clip_min_y: usize,
    clip_max_x: usize,
    clip_max_y: usize,
    q: &AlignedTexQuad,
    tex: &CpuTexture,
) {
    if tex.width == 0 || tex.height == 0 {
        return;
    }
    let px0 = ((q.x0_f - 0.5).ceil().max(clip_min_x as f32)) as usize;
    let px1 = ((q.x1_f - 0.5).ceil().min(clip_max_x as f32)) as usize;
    let py0 = ((q.y0_f - 0.5).ceil().max(clip_min_y as f32)) as usize;
    let py1 = ((q.y1_f - 0.5).ceil().min(clip_max_y as f32)) as usize;
    if px0 >= px1 || py0 >= py1 {
        return;
    }

    let inv_w = 1.0 / (q.x1_f - q.x0_f).max(1e-5);
    let inv_h = 1.0 / (q.y1_f - q.y0_f).max(1e-5);
    let du_dx = (q.u1 - q.u0) * inv_w;
    let dv_dy = (q.v1 - q.v0) * inv_h;
    let tex_wf = tex.width as f32;
    let tex_hf = tex.height as f32;
    let dufx_dx = du_dx * tex_wf;
    let dvfy_dy = dv_dy * tex_hf;
    let max_tx = (tex.width - 1) as f32;
    let max_ty = (tex.height - 1) as f32;

    let ufx_start = ((q.u0 + ((px0 as f32 + 0.5) - q.x0_f) * du_dx).clamp(0.0, 1.0) * tex_wf - 0.5)
        .clamp(0.0, max_tx);
    let vfy_start = ((q.v0 + ((py0 as f32 + 0.5) - q.y0_f) * dv_dy).clamp(0.0, 1.0) * tex_hf - 0.5)
        .clamp(0.0, max_ty);

    let cr = q.col[0] as u32;
    let cg = q.col[1] as u32;
    let cb = q.col[2] as u32;
    let ca = q.col[3] as u32;

    let aligned_1_to_1 = (dufx_dx - 1.0).abs() < 2e-3
        && (dvfy_dy - 1.0).abs() < 2e-3
        && (ufx_start - ufx_start.round()).abs() < 0.03
        && (vfy_start - vfy_start.round()).abs() < 0.03;

    if aligned_1_to_1 {
        let tx0 = (ufx_start.round() as usize).min(tex.width - 1);
        let ty0 = (vfy_start.round() as usize).min(tex.height - 1);
        for (dy, py) in (py0..py1).enumerate() {
            let sy = (ty0 + dy).min(tex.height - 1);
            let tex_row = &tex.pixels[sy * tex.width..];
            let dst_row = &mut fb[py * fb_stride..];
            for (dx, px) in (px0..px1).enumerate() {
                let tc = tex_row[(tx0 + dx).min(tex.width - 1)];
                let a = tc[3] as u32;
                if a == 0 {
                    continue;
                }
                let sa = div255(ca * a);
                if sa == 0 {
                    continue;
                }
                let sr = div255(cr * tc[0] as u32);
                let sg = div255(cg * tc[1] as u32);
                let sb = div255(cb * tc[2] as u32);
                if sa == 255 {
                    dst_row[px] = (sr << 16) | (sg << 8) | sb;
                } else {
                    let dst = dst_row[px];
                    let dr = (dst >> 16) & 0xFF;
                    let dg = (dst >> 8) & 0xFF;
                    let db = dst & 0xFF;
                    let inv_a = 255 - sa;
                    let r = (sr + div255(dr * inv_a)).min(255);
                    let g = (sg + div255(dg * inv_a)).min(255);
                    let b = (sb + div255(db * inv_a)).min(255);
                    dst_row[px] = (r << 16) | (g << 8) | b;
                }
            }
        }
        return;
    }

    for (dy, py) in (py0..py1).enumerate() {
        let vfy = (vfy_start + (dy as f32) * dvfy_dy).clamp(0.0, max_ty);
        let y0 = vfy.floor() as usize;
        let y1 = (y0 + 1).min(tex.height - 1);
        let ty = vfy - y0 as f32;
        let wy1 = (ty * 256.0 + 0.5) as u32;
        let wy0 = 256 - wy1;
        let row0 = &tex.pixels[y0 * tex.width..];
        let row1 = &tex.pixels[y1 * tex.width..];
        let dst_row = &mut fb[py * fb_stride..];

        let mut ufx = ufx_start;
        for px in px0..px1 {
            let ux = ufx.clamp(0.0, max_tx);
            let x0 = ux.floor() as usize;
            let x1 = (x0 + 1).min(tex.width - 1);
            let wx1 = ((ux - x0 as f32) * 256.0 + 0.5) as u32;
            let wx0 = 256 - wx1;

            let p00 = row0[x0];
            let p10 = row0[x1];
            let p01 = row1[x0];
            let p11 = row1[x1];

            let w00 = wx0 * wy0;
            let w10 = wx1 * wy0;
            let w01 = wx0 * wy1;
            let w11 = wx1 * wy1;

            let ta = (p00[3] as u32 * w00 + p10[3] as u32 * w10 + p01[3] as u32 * w01 + p11[3] as u32 * w11 + 32768) >> 16;
            if ta > 0 {
                let tr = (p00[0] as u32 * w00 + p10[0] as u32 * w10 + p01[0] as u32 * w01 + p11[0] as u32 * w11 + 32768) >> 16;
                let tg = (p00[1] as u32 * w00 + p10[1] as u32 * w10 + p01[1] as u32 * w01 + p11[1] as u32 * w11 + 32768) >> 16;
                let tb = (p00[2] as u32 * w00 + p10[2] as u32 * w10 + p01[2] as u32 * w01 + p11[2] as u32 * w11 + 32768) >> 16;
                let sr = div255(cr * tr);
                let sg = div255(cg * tg);
                let sb = div255(cb * tb);
                let sa = div255(ca * ta);
                if sa == 255 {
                    dst_row[px] = (sr << 16) | (sg << 8) | sb;
                } else if sa > 0 {
                    let dst = dst_row[px];
                    let dr = (dst >> 16) & 0xFF;
                    let dg = (dst >> 8) & 0xFF;
                    let db = dst & 0xFF;
                    let inv_a = 255 - sa;
                    let r = (sr + div255(dr * inv_a)).min(255);
                    let g = (sg + div255(dg * inv_a)).min(255);
                    let b = (sb + div255(db * inv_a)).min(255);
                    dst_row[px] = (r << 16) | (g << 8) | b;
                }
            }
            ufx += dufx_dx;
        }
    }
}

#[inline]
fn detect_solid_quad(
    verts: &[egui::epaint::Vertex],
    idx6: &[u32],
    ppp: f32,
) -> Option<(usize, usize, usize, usize, [u8; 4])> {
    let v0 = &verts[idx6[0] as usize];
    if v0.uv != WHITE_UV {
        return None;
    }
    let c = v0.color.to_array();
    let v1 = &verts[idx6[1] as usize];
    if v1.uv != WHITE_UV || v1.color.to_array() != c {
        return None;
    }
    let mut xs = [f32::INFINITY, f32::NEG_INFINITY];
    let mut ys = [f32::INFINITY, f32::NEG_INFINITY];
    for &idx in idx6 {
        let v = &verts[idx as usize];
        if v.uv != WHITE_UV || v.color.to_array() != c {
            return None;
        }
        xs[0] = xs[0].min(v.pos.x);
        xs[1] = xs[1].max(v.pos.x);
        ys[0] = ys[0].min(v.pos.y);
        ys[1] = ys[1].max(v.pos.y);
    }
    for &idx in idx6 {
        let v = &verts[idx as usize];
        let on_x = (v.pos.x - xs[0]).abs() < 1e-3 || (v.pos.x - xs[1]).abs() < 1e-3;
        let on_y = (v.pos.y - ys[0]).abs() < 1e-3 || (v.pos.y - ys[1]).abs() < 1e-3;
        if !(on_x && on_y) {
            return None;
        }
    }
    let px0 = (xs[0] * ppp).round().max(0.0) as usize;
    let py0 = (ys[0] * ppp).round().max(0.0) as usize;
    let px1 = (xs[1] * ppp).round().max(0.0) as usize;
    let py1 = (ys[1] * ppp).round().max(0.0) as usize;
    if px1 <= px0 || py1 <= py0 {
        return None;
    }
    Some((px0, py0, px1, py1, c))
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn fill_solid_rect(
    fb: &mut [u32],
    fb_stride: usize,
    clip_min_x: usize,
    clip_min_y: usize,
    clip_max_x: usize,
    clip_max_y: usize,
    rx0: usize,
    ry0: usize,
    rx1: usize,
    ry1: usize,
    src: [u8; 4],
) {
    if src[3] == 0 {
        return;
    }
    let x0 = rx0.max(clip_min_x);
    let y0 = ry0.max(clip_min_y);
    let x1 = rx1.min(clip_max_x);
    let y1 = ry1.min(clip_max_y);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    if src[3] == 255 {
        let packed = ((src[0] as u32) << 16) | ((src[1] as u32) << 8) | (src[2] as u32);
        for py in y0..y1 {
            let row = py * fb_stride;
            fb[row + x0..row + x1].fill(packed);
        }
    } else {
        blend_premultiplied_span_rows(fb, fb_stride, x0, x1, y0, y1, src);
    }
}

#[inline]
fn blend_premultiplied_span_rows(
    fb: &mut [u32],
    fb_stride: usize,
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    src: [u8; 4],
) {
    let sr = src[0] as u32;
    let sg = src[1] as u32;
    let sb = src[2] as u32;
    let inv_a = 255 - src[3] as u32;
    for py in y0..y1 {
        let row = py * fb_stride;
        for dst in &mut fb[row + x0..row + x1] {
            let d = *dst;
            let dr = (d >> 16) & 0xFF;
            let dg = (d >> 8) & 0xFF;
            let db = d & 0xFF;
            let r = (sr + div255(dr * inv_a)).min(255);
            let g = (sg + div255(dg * inv_a)).min(255);
            let b = (sb + div255(db * inv_a)).min(255);
            *dst = (r << 16) | (g << 8) | b;
        }
    }
}

fn decode_image(img: &ImageData) -> (usize, usize, Vec<[u8; 4]>) {
    match img {
        ImageData::Color(c) => {
            let pixels = c.pixels.iter().map(Color32::to_array).collect();
            (c.size[0], c.size[1], pixels)
        }
        ImageData::Font(f) => {
            let pixels = f.srgba_pixels(None).map(|p| p.to_array()).collect();
            (f.size[0], f.size[1], pixels)
        }
    }
}

#[inline]
fn edge_pass(w: f32, ex: f32, ey: f32) -> bool {
    if w > 0.0 {
        true
    } else if w < 0.0 {
        false
    } else {
        ey < 0.0 || (ey == 0.0 && ex < 0.0)
    }
}

#[inline]
fn precompute_inv_slopes(ey: [f32; 3]) -> [f32; 3] {
    [
        if ey[0].abs() > 1e-6 { -1.0 / ey[0] } else { 0.0 },
        if ey[1].abs() > 1e-6 { -1.0 / ey[1] } else { 0.0 },
        if ey[2].abs() > 1e-6 { -1.0 / ey[2] } else { 0.0 },
    ]
}

/// Computes the exact horizontal `[span_x0 .. span_x1)` covered on scanline `fy` matching `edge_pass`.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
#[inline]
fn scanline_x_span(
    min_x: usize,
    max_x: usize,
    fy: f32,
    ex: [f32; 3],
    ey: [f32; 3],
    vx: [f32; 3],
    vy: [f32; 3],
) -> Option<(usize, usize)> {
    scanline_x_span_with_inv(min_x, max_x, fy, ex, ey, precompute_inv_slopes(ey), vx, vy)
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn scanline_x_span_with_inv(
    min_x: usize,
    max_x: usize,
    fy: f32,
    ex: [f32; 3],
    ey: [f32; 3],
    inv_neg_ey: [f32; 3],
    vx: [f32; 3],
    vy: [f32; 3],
) -> Option<(usize, usize)> {
    let mut lo = min_x as f32;
    let mut hi = max_x as f32;
    // Edge k passes through vertex p[(k+1)%3] with direction (ex[k], ey[k])
    let c = [
        ex[0] * (fy - vy[1]) + ey[0] * (vx[1] - 0.5),
        ex[1] * (fy - vy[2]) + ey[1] * (vx[2] - 0.5),
        ex[2] * (fy - vy[0]) + ey[2] * (vx[0] - 0.5),
    ];
    for k in 0..3 {
        let inv = inv_neg_ey[k];
        if inv > 0.0 {
            let x_enter = (-c[k] * inv).floor() - 1.0;
            lo = lo.max(x_enter);
        } else if inv < 0.0 {
            let x_exit = (-c[k] * inv).ceil() + 2.0;
            hi = hi.min(x_exit);
        } else if !edge_pass(c[k], ex[k], ey[k]) {
            return None;
        }
    }
    let mut sx0 = (lo.max(min_x as f32) as usize).min(max_x);
    let mut sx1 = (hi.min(max_x as f32) as usize).max(sx0);
    // Tighten up to 2 boundary pixels using exact edge_pass so every pixel matches scalar evaluation bit-for-bit.
    while sx0 < sx1 {
        let px = sx0 as f32;
        if edge_pass(c[0] - ey[0] * px, ex[0], ey[0])
            && edge_pass(c[1] - ey[1] * px, ex[1], ey[1])
            && edge_pass(c[2] - ey[2] * px, ex[2], ey[2])
        {
            break;
        }
        sx0 += 1;
    }
    while sx1 > sx0 {
        let px = (sx1 - 1) as f32;
        if edge_pass(c[0] - ey[0] * px, ex[0], ey[0])
            && edge_pass(c[1] - ey[1] * px, ex[1], ey[1])
            && edge_pass(c[2] - ey[2] * px, ex[2], ey[2])
        {
            break;
        }
        sx1 -= 1;
    }
    (sx0 < sx1).then_some((sx0, sx1))
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn rasterize_triangle(
    fb: &mut [u32],
    fb_stride: usize,
    clip_min_x: usize,
    clip_min_y: usize,
    clip_max_x: usize,
    clip_max_y: usize,
    ppp: f32,
    v0: &egui::epaint::Vertex,
    v1: &egui::epaint::Vertex,
    v2: &egui::epaint::Vertex,
    tex: Option<&CpuTexture>,
) {
    let x0 = v0.pos.x * ppp;
    let y0 = v0.pos.y * ppp;
    let x1 = v1.pos.x * ppp;
    let y1 = v1.pos.y * ppp;
    let x2 = v2.pos.x * ppp;
    let y2 = v2.pos.y * ppp;

    let mut area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
    if area.abs() < 1e-4 {
        return;
    }

    let min_x = x0.min(x1).min(x2).floor().max(clip_min_x as f32) as usize;
    let max_x = x0.max(x1).max(x2).ceil().min(clip_max_x as f32) as usize;
    let min_y = y0.min(y1).min(y2).floor().max(clip_min_y as f32) as usize;
    let max_y = y0.max(y1).max(y2).ceil().min(clip_max_y as f32) as usize;
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let sign = if area < 0.0 {
        area = -area;
        -1.0f32
    } else {
        1.0f32
    };
    let inv_area = 1.0 / area;

    let ex = [(x2 - x1) * sign, (x0 - x2) * sign, (x1 - x0) * sign];
    let ey = [(y2 - y1) * sign, (y0 - y2) * sign, (y1 - y0) * sign];
    let inv_neg_ey = precompute_inv_slopes(ey);
    let vx = [x0, x1, x2];
    let vy = [y0, y1, y2];

    let c0 = v0.color.to_array();
    let c1 = v1.color.to_array();
    let c2 = v2.color.to_array();

    let uniform_col = c0 == c1 && c1 == c2;
    let untextured = v0.uv == WHITE_UV && v1.uv == WHITE_UV && v2.uv == WHITE_UV;

    if uniform_col && untextured {
        if c0[3] == 0 {
            return;
        }
        if c0[3] == 255 {
            let packed = ((c0[0] as u32) << 16) | ((c0[1] as u32) << 8) | (c0[2] as u32);
            for py in min_y..max_y {
                let fy = py as f32 + 0.5;
                if let Some((sx0, sx1)) =
                    scanline_x_span_with_inv(min_x, max_x, fy, ex, ey, inv_neg_ey, vx, vy)
                {
                    let row_off = py * fb_stride;
                    fb[row_off + sx0..row_off + sx1].fill(packed);
                }
            }
        } else {
            for py in min_y..max_y {
                let fy = py as f32 + 0.5;
                if let Some((sx0, sx1)) =
                    scanline_x_span_with_inv(min_x, max_x, fy, ex, ey, inv_neg_ey, vx, vy)
                {
                    blend_premultiplied_span_rows(fb, fb_stride, sx0, sx1, py, py + 1, c0);
                }
            }
        }
        return;
    }

    // Barycentric horizontal increments: db_k / d(px) = -ey[k] * inv_area
    let db0_dx = -ey[0] * inv_area;
    let db1_dx = -ey[1] * inv_area;

    if untextured {
        if c0[3] == 0 && c1[3] == 0 && c2[3] == 0 {
            return;
        }
        let c0f = [c0[0] as f32, c0[1] as f32, c0[2] as f32, c0[3] as f32];
        let c1f = [c1[0] as f32, c1[1] as f32, c1[2] as f32, c1[3] as f32];
        let c2f = [c2[0] as f32, c2[1] as f32, c2[2] as f32, c2[3] as f32];
        let dr_dx = (c0f[0] - c2f[0]) * db0_dx + (c1f[0] - c2f[0]) * db1_dx;
        let dg_dx = (c0f[1] - c2f[1]) * db0_dx + (c1f[1] - c2f[1]) * db1_dx;
        let db_dx = (c0f[2] - c2f[2]) * db0_dx + (c1f[2] - c2f[2]) * db1_dx;
        let da_dx = (c0f[3] - c2f[3]) * db0_dx + (c1f[3] - c2f[3]) * db1_dx;

        for py in min_y..max_y {
            let fy = py as f32 + 0.5;
            let Some((sx0, sx1)) =
                scanline_x_span_with_inv(min_x, max_x, fy, ex, ey, inv_neg_ey, vx, vy)
            else {
                continue;
            };
            let row_off = py * fb_stride;
            let fx0 = sx0 as f32 + 0.5;
            let b0 = ((ex[0] * (fy - y1) - ey[0] * (fx0 - x1)) * inv_area).clamp(0.0, 1.0);
            let b1 = ((ex[1] * (fy - y2) - ey[1] * (fx0 - x2)) * inv_area).clamp(0.0, 1.0);
            let b2 = (1.0 - b0 - b1).max(0.0);

            let mut r = c0f[0] * b0 + c1f[0] * b1 + c2f[0] * b2;
            let mut g = c0f[1] * b0 + c1f[1] * b1 + c2f[1] * b2;
            let mut b = c0f[2] * b0 + c1f[2] * b1 + c2f[2] * b2;
            let mut a = c0f[3] * b0 + c1f[3] * b1 + c2f[3] * b2;

            for px in sx0..sx1 {
                let ca = (a + 0.5).clamp(0.0, 255.0) as u32;
                if ca > 0 {
                    let cr = (r + 0.5).clamp(0.0, 255.0) as u32;
                    let cg = (g + 0.5).clamp(0.0, 255.0) as u32;
                    let cb = (b + 0.5).clamp(0.0, 255.0) as u32;
                    if ca == 255 {
                        fb[row_off + px] = (cr << 16) | (cg << 8) | cb;
                    } else {
                        let dst = fb[row_off + px];
                        let dr = (dst >> 16) & 0xFF;
                        let dg = (dst >> 8) & 0xFF;
                        let db = dst & 0xFF;
                        let inv_a = 255 - ca;
                        let or = (cr + div255(dr * inv_a)).min(255);
                        let og = (cg + div255(dg * inv_a)).min(255);
                        let ob = (cb + div255(db * inv_a)).min(255);
                        fb[row_off + px] = (or << 16) | (og << 8) | ob;
                    }
                }
                r += dr_dx;
                g += dg_dx;
                b += db_dx;
                a += da_dx;
            }
        }
        return;
    }

    for py in min_y..max_y {
        let fy = py as f32 + 0.5;
        let Some((sx0, sx1)) =
            scanline_x_span_with_inv(min_x, max_x, fy, ex, ey, inv_neg_ey, vx, vy)
        else {
            continue;
        };
        let row_off = py * fb_stride;
        let fx0 = sx0 as f32 + 0.5;
        let mut b0 = (ex[0] * (fy - y1) - ey[0] * (fx0 - x1)) * inv_area;
        let mut b1 = (ex[1] * (fy - y2) - ey[1] * (fx0 - x2)) * inv_area;

        for px in sx0..sx1 {
            let cb0 = b0.clamp(0.0, 1.0);
            let cb1 = b1.clamp(0.0, 1.0);
            let cb2 = (1.0 - cb0 - cb1).max(0.0);

            let (vr, vg, vb, va) = if uniform_col {
                (c0[0], c0[1], c0[2], c0[3])
            } else {
                (
                    (c0[0] as f32 * cb0 + c1[0] as f32 * cb1 + c2[0] as f32 * cb2 + 0.5) as u8,
                    (c0[1] as f32 * cb0 + c1[1] as f32 * cb1 + c2[1] as f32 * cb2 + 0.5) as u8,
                    (c0[2] as f32 * cb0 + c1[2] as f32 * cb1 + c2[2] as f32 * cb2 + 0.5) as u8,
                    (c0[3] as f32 * cb0 + c1[3] as f32 * cb1 + c2[3] as f32 * cb2 + 0.5) as u8,
                )
            };

            let src = if untextured {
                [vr, vg, vb, va]
            } else if let Some(t) = tex {
                let u = (v0.uv.x * cb0 + v1.uv.x * cb1 + v2.uv.x * cb2).clamp(0.0, 1.0);
                let v = (v0.uv.y * cb0 + v1.uv.y * cb1 + v2.uv.y * cb2).clamp(0.0, 1.0);
                let tc = sample_bilinear(t, u, v);
                if tc[3] == 0 {
                    b0 += db0_dx;
                    b1 += db1_dx;
                    continue;
                }
                [
                    ((vr as u16 * tc[0] as u16 + 127) / 255) as u8,
                    ((vg as u16 * tc[1] as u16 + 127) / 255) as u8,
                    ((vb as u16 * tc[2] as u16 + 127) / 255) as u8,
                    ((va as u16 * tc[3] as u16 + 127) / 255) as u8,
                ]
            } else {
                [vr, vg, vb, va]
            };

            b0 += db0_dx;
            b1 += db1_dx;

            if src[3] == 0 {
                continue;
            }
            if src[3] == 255 {
                fb[row_off + px] =
                    ((src[0] as u32) << 16) | ((src[1] as u32) << 8) | (src[2] as u32);
            } else {
                let dst = fb[row_off + px];
                let dr = (dst >> 16) & 0xFF;
                let dg = (dst >> 8) & 0xFF;
                let db = dst & 0xFF;
                let inv_a = 255 - src[3] as u32;
                let r = (src[0] as u32 + div255(dr * inv_a)).min(255);
                let g = (src[1] as u32 + div255(dg * inv_a)).min(255);
                let b = (src[2] as u32 + div255(db * inv_a)).min(255);
                fb[row_off + px] = (r << 16) | (g << 8) | b;
            }
        }
    }
}

#[inline]
fn sample_bilinear(tex: &CpuTexture, u: f32, v: f32) -> [u8; 4] {
    if tex.width == 0 || tex.height == 0 {
        return [255, 255, 255, 255];
    }
    let fx = (u * tex.width as f32 - 0.5).clamp(0.0, (tex.width - 1) as f32);
    let fy = (v * tex.height as f32 - 0.5).clamp(0.0, (tex.height - 1) as f32);
    let x0 = fx.floor() as usize;
    let y0 = fy.floor() as usize;
    let x1 = (x0 + 1).min(tex.width - 1);
    let y1 = (y0 + 1).min(tex.height - 1);
    let tx = (fx - x0 as f32).clamp(0.0, 1.0);
    let ty = (fy - y0 as f32).clamp(0.0, 1.0);

    let p00 = tex.pixels[y0 * tex.width + x0];
    let p10 = tex.pixels[y0 * tex.width + x1];
    let p01 = tex.pixels[y1 * tex.width + x0];
    let p11 = tex.pixels[y1 * tex.width + x1];

    let w00 = (1.0 - tx) * (1.0 - ty);
    let w10 = tx * (1.0 - ty);
    let w01 = (1.0 - tx) * ty;
    let w11 = tx * ty;

    [
        (p00[0] as f32 * w00 + p10[0] as f32 * w10 + p01[0] as f32 * w01 + p11[0] as f32 * w11 + 0.5)
            as u8,
        (p00[1] as f32 * w00 + p10[1] as f32 * w10 + p01[1] as f32 * w01 + p11[1] as f32 * w11 + 0.5)
            as u8,
        (p00[2] as f32 * w00 + p10[2] as f32 * w10 + p01[2] as f32 * w01 + p11[2] as f32 * w11 + 0.5)
            as u8,
        (p00[3] as f32 * w00 + p10[3] as f32 * w10 + p01[3] as f32 * w01 + p11[3] as f32 * w11 + 0.5)
            as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, RawInput, Rect, Vec2, ViewportId};

    #[test]
    fn software_painter_scales_crisp_at_150pct_dpi() {
        let render = |ppp: f32, w: usize, h: usize| -> Vec<u32> {
            let ctx = egui::Context::default();
            let mut raw = RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(w as f32 / ppp, h as f32 / ppp),
                )),
                ..Default::default()
            };
            raw.viewports
                .entry(ViewportId::ROOT)
                .or_default()
                .native_pixels_per_point = Some(ppp);
            let out = ctx.run(raw, |c| {
                egui::CentralPanel::default().show(c, |ui| {
                    ui.heading("y-storage DPI Scaling");
                    ui.label("Crisp pastel dark rasterization");
                });
            });
            let mut p = SoftwarePainter::new();
            p.apply_textures(&out.textures_delta);
            let clipped = ctx.tessellate(out.shapes, out.pixels_per_point);
            p.paint_and_blit(null_mut(), w, h, out.pixels_per_point, &clipped, 0x000A_0A10, None);
            p.backbuffer_pixels().to_vec()
        };

        let fb100 = render(1.0, 400, 300);
        let fb150 = render(1.5, 600, 450);
        let lit100 = fb100.iter().filter(|&&px| px != 0x000A_0A10 && px != 0x001B_1B1B).count();
        let lit150 = fb150.iter().filter(|&&px| px != 0x000A_0A10 && px != 0x001B_1B1B).count();
        assert!(lit150 * 10 > lit100 * 17);
    }

    #[test]
    fn scanline_x_span_matches_scalar_edge_pass_for_diagonal_triangles() {
        let verts = [
            egui::epaint::Vertex {
                pos: Pos2::new(10.3, 8.7),
                uv: WHITE_UV,
                color: Color32::from_rgba_unmultiplied(84, 185, 242, 200),
            },
            egui::epaint::Vertex {
                pos: Pos2::new(93.8, 22.1),
                uv: WHITE_UV,
                color: Color32::from_rgba_unmultiplied(216, 166, 255, 200),
            },
            egui::epaint::Vertex {
                pos: Pos2::new(34.2, 67.9),
                uv: WHITE_UV,
                color: Color32::from_rgba_unmultiplied(247, 168, 216, 200),
            },
        ];
        let x0 = verts[0].pos.x;
        let y0 = verts[0].pos.y;
        let x1 = verts[1].pos.x;
        let y1 = verts[1].pos.y;
        let x2 = verts[2].pos.x;
        let y2 = verts[2].pos.y;
        let area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        let sign = if area < 0.0 { -1.0 } else { 1.0 };
        let ex = [(x2 - x1) * sign, (x0 - x2) * sign, (x1 - x0) * sign];
        let ey = [(y2 - y1) * sign, (y0 - y2) * sign, (y1 - y0) * sign];
        let vx = [x0, x1, x2];
        let vy = [y0, y1, y2];

        for py in 0..80usize {
            let fy = py as f32 + 0.5;
            let fast = scanline_x_span(0, 120, fy, ex, ey, vx, vy);
            for px in 0..120usize {
                let fx = px as f32 + 0.5;
                let inside_scalar = edge_pass(ex[0] * (fy - y1) - ey[0] * (fx - x1), ex[0], ey[0])
                    && edge_pass(ex[1] * (fy - y2) - ey[1] * (fx - x2), ex[1], ey[1])
                    && edge_pass(ex[2] * (fy - y0) - ey[2] * (fx - x0), ex[2], ey[2]);
                let inside_fast = fast.is_some_and(|(a, b)| px >= a && px < b);
                assert_eq!(inside_fast, inside_scalar, "mismatch at ({px}, {py})");
            }
        }
    }

    #[test]
    fn aligned_tex_and_solid_quads_recover_alignment_after_single_triangle() {
        let mut p = SoftwarePainter::new();
        let tex_id = TextureId::Managed(1);
        let font_pixels = vec![
            Color32::from_rgba_premultiplied(255, 255, 255, 255),
            Color32::from_rgba_premultiplied(128, 128, 128, 128),
            Color32::from_rgba_premultiplied(0, 0, 0, 0),
            Color32::from_rgba_premultiplied(200, 200, 200, 200),
        ];
        p.apply_textures(&TexturesDelta {
            set: vec![(
                tex_id,
                egui::epaint::ImageDelta::full(
                    ImageData::Color(std::sync::Arc::new(egui::ColorImage {
                        size: [2, 2],
                        pixels: font_pixels,
                    })),
                    egui::TextureOptions::NEAREST,
                ),
            )],
            free: vec![],
        });

        let mut mesh = egui::epaint::Mesh::with_texture(tex_id);
        let tri_col = Color32::from_rgb(200, 40, 40);
        // Preceding 3-index triangle that would shift naive 6-index quad loops off by 3 indices.
        mesh.vertices.extend_from_slice(&[
            egui::epaint::Vertex { pos: Pos2::new(2.0, 2.0), uv: WHITE_UV, color: tri_col },
            egui::epaint::Vertex { pos: Pos2::new(6.0, 2.0), uv: WHITE_UV, color: tri_col },
            egui::epaint::Vertex { pos: Pos2::new(4.0, 6.0), uv: WHITE_UV, color: tri_col },
        ]);
        mesh.indices.extend_from_slice(&[0, 1, 2]);

        // Followed immediately by a 6-index textured glyph quad and a 6-index solid quad.
        let tint = Color32::from_rgb(100, 200, 250);
        let b = mesh.vertices.len() as u32;
        mesh.vertices.extend_from_slice(&[
            egui::epaint::Vertex { pos: Pos2::new(10.0, 10.0), uv: Pos2::new(0.0, 0.0), color: tint },
            egui::epaint::Vertex { pos: Pos2::new(12.0, 10.0), uv: Pos2::new(1.0, 0.0), color: tint },
            egui::epaint::Vertex { pos: Pos2::new(12.0, 12.0), uv: Pos2::new(1.0, 1.0), color: tint },
            egui::epaint::Vertex { pos: Pos2::new(10.0, 12.0), uv: Pos2::new(0.0, 1.0), color: tint },
        ]);
        mesh.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);

        let sb = mesh.vertices.len() as u32;
        let sq_col = Color32::from_rgb(30, 210, 90);
        mesh.vertices.extend_from_slice(&[
            egui::epaint::Vertex { pos: Pos2::new(20.0, 10.0), uv: WHITE_UV, color: sq_col },
            egui::epaint::Vertex { pos: Pos2::new(24.0, 10.0), uv: WHITE_UV, color: sq_col },
            egui::epaint::Vertex { pos: Pos2::new(24.0, 14.0), uv: WHITE_UV, color: sq_col },
            egui::epaint::Vertex { pos: Pos2::new(20.0, 14.0), uv: WHITE_UV, color: sq_col },
        ]);
        mesh.indices.extend_from_slice(&[sb, sb + 1, sb + 2, sb, sb + 2, sb + 3]);

        let clipped = [ClippedPrimitive {
            clip_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 30.0)),
            primitive: Primitive::Mesh(mesh),
        }];
        let mut stats = PaintStats::default();
        p.paint_and_blit_profiled(null_mut(), 40, 30, 1.0, &clipped, 0, None, Some(&mut stats));
        assert_eq!(stats.solid_quads, 1);
        assert_eq!(stats.textured_tri, 2);
        let fb = p.backbuffer_pixels();
        assert_eq!(fb[10 * 40 + 10], ((100u32) << 16) | ((200u32) << 8) | 250);
        assert_eq!(fb[11 * 40 + 10], 0);
        assert_eq!(fb[12 * 40 + 22], ((30u32) << 16) | ((210u32) << 8) | 90);
    }
}
