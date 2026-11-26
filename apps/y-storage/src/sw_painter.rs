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
        if width == 0 || height == 0 {
            return;
        }
        let stride = self.ensure_backbuffer(hdc, width, height);
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
                let tex = self.textures.get(&mesh.texture_id);
                let mut i = 0usize;
                while i + 6 <= mesh.indices.len() {
                    let t0 = &mesh.indices[i..i + 6];
                    if let Some((qx0, qy0, qx1, qy1, col)) =
                        detect_solid_quad(&mesh.vertices, t0, pixels_per_point)
                    {
                        fill_solid_rect(
                            fb, stride, cmin_x, cmin_y, cmax_x, cmax_y, qx0, qy0, qx1, qy1, col,
                        );
                    } else {
                        for k in 0..2 {
                            let tri = &mesh.indices[i + k * 3..i + k * 3 + 3];
                            rasterize_triangle(
                                fb,
                                stride,
                                cmin_x,
                                cmin_y,
                                cmax_x,
                                cmax_y,
                                pixels_per_point,
                                &mesh.vertices[tri[0] as usize],
                                &mesh.vertices[tri[1] as usize],
                                &mesh.vertices[tri[2] as usize],
                                tex,
                            );
                        }
                    }
                    i += 6;
                }
                while i + 3 <= mesh.indices.len() {
                    let tri = &mesh.indices[i..i + 3];
                    rasterize_triangle(
                        fb,
                        stride,
                        cmin_x,
                        cmin_y,
                        cmax_x,
                        cmax_y,
                        pixels_per_point,
                        &mesh.vertices[tri[0] as usize],
                        &mesh.vertices[tri[1] as usize],
                        &mesh.vertices[tri[2] as usize],
                        tex,
                    );
                    i += 3;
                }
            }
        }

        if !hdc.is_null() {
            self.present_rect(hdc, present_clip);
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
            let r = (sr + ((dr * inv_a + 127) / 255)).min(255);
            let g = (sg + ((dg * inv_a + 127) / 255)).min(255);
            let b = (sb + ((db * inv_a + 127) / 255)).min(255);
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

/// Computes the exact horizontal `[span_x0 .. span_x1)` covered on scanline `fy` matching `edge_pass`.
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
    let mut lo = min_x as f32;
    let mut hi = max_x as f32;
    // Edge k passes through vertex p[(k+1)%3] with direction (ex[k], ey[k])
    let c = [
        ex[0] * (fy - vy[1]) + ey[0] * (vx[1] - 0.5),
        ex[1] * (fy - vy[2]) + ey[1] * (vx[2] - 0.5),
        ex[2] * (fy - vy[0]) + ey[2] * (vx[0] - 0.5),
    ];
    for k in 0..3 {
        let slope = -ey[k];
        if slope > 1e-6 {
            let x_enter = (-c[k] / slope).floor() - 1.0;
            lo = lo.max(x_enter);
        } else if slope < -1e-6 {
            let x_exit = (-c[k] / slope).ceil() + 2.0;
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
                if let Some((sx0, sx1)) = scanline_x_span(min_x, max_x, fy, ex, ey, vx, vy) {
                    let row_off = py * fb_stride;
                    fb[row_off + sx0..row_off + sx1].fill(packed);
                }
            }
        } else {
            for py in min_y..max_y {
                let fy = py as f32 + 0.5;
                if let Some((sx0, sx1)) = scanline_x_span(min_x, max_x, fy, ex, ey, vx, vy) {
                    blend_premultiplied_span_rows(fb, fb_stride, sx0, sx1, py, py + 1, c0);
                }
            }
        }
        return;
    }

    // Barycentric horizontal increments: db_k / d(px) = -ey[k] * inv_area
    let db0_dx = -ey[0] * inv_area;
    let db1_dx = -ey[1] * inv_area;

    for py in min_y..max_y {
        let fy = py as f32 + 0.5;
        let Some((sx0, sx1)) = scanline_x_span(min_x, max_x, fy, ex, ey, vx, vy) else {
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
                let r = (src[0] as u32 + ((dr * inv_a + 127) / 255)).min(255);
                let g = (src[1] as u32 + ((dg * inv_a + 127) / 255)).min(255);
                let b = (src[2] as u32 + ((db * inv_a + 127) / 255)).min(255);
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
}
