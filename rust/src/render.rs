//! Batched 2D renderer. Every primitive is a quad evaluated by one signed-distance-field
//! shader, so cartoon shapes (rounded boxes, ellipses, pies, glows) and outlined text are
//! anti-aliased at any resolution without bitmap assets. One draw call per frame in practice.
#![allow(dead_code, clippy::too_many_arguments)]

use crate::gl::*;
use crate::math::*;
use std::ffi::CString;

pub const VIRTUAL_H: f32 = 720.0;
const MAX_QUADS: usize = 16000;
const SDF_SPREAD: f32 = 8.0;

const K_RRECT: f32 = 0.0;
const K_ELLIPSE: f32 = 1.0;
const K_TEXT: f32 = 2.0;
const K_SOLID: f32 = 3.0;
const K_PIE: f32 = 4.0;
const K_GLOW: f32 = 5.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    local: [f32; 2],
    shape: [f32; 4],
    fill: [u8; 4],
    line: [u8; 4],
    kind: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Glyph {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub bx: f32,
    pub by: f32,
    pub adv: f32,
}

pub struct Font {
    pub glyphs: Vec<Glyph>,
    pub px: f32,
    pub tex_w: usize,
    pub tex_h: usize,
    pub sdf: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// 2D affine transform: (x, y) -> (a x + c y + tx, b x + d y + ty).
#[derive(Clone, Copy)]
struct Aff {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    tx: f32,
    ty: f32,
}

impl Aff {
    const ID: Aff = Aff { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };
    fn apply(&self, p: V2) -> V2 {
        v2(self.a * p.x + self.c * p.y + self.tx, self.b * p.x + self.d * p.y + self.ty)
    }
    fn mul(&self, o: &Aff) -> Aff {
        Aff {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            tx: self.a * o.tx + self.c * o.ty + self.tx,
            ty: self.b * o.tx + self.d * o.ty + self.ty,
        }
    }
}

struct GlState {
    prog: GLuint,
    vao: GLuint,
    vbo: GLuint,
    ibo: GLuint,
    tex: GLuint,
    u_scale: GLint,
    u_tex: GLint,
    u_texsize: GLint,
}

pub struct Renderer {
    verts: Vec<Vertex>,
    gl: Option<GlState>,
    stack: Vec<Aff>,
    m: Aff,
    alpha: f32,
    alpha_stack: Vec<f32>,
    pub font: Font,
    /// Virtual canvas size (height is always 720).
    pub w: f32,
    pub h: f32,
    pub px_w: f32,
    pub px_h: f32,
    pub draw_calls: u32,
}

const VS: &str = r#"#version 300 es
layout(location=0) in vec2 a_pos;
layout(location=1) in vec2 a_local;
layout(location=2) in vec4 a_shape;
layout(location=3) in vec4 a_fill;
layout(location=4) in vec4 a_line;
layout(location=5) in float a_kind;
uniform vec2 u_scale;
out vec2 v_local;
out vec4 v_shape;
out vec4 v_fill;
out vec4 v_line;
flat out int v_kind;
void main() {
    gl_Position = vec4(a_pos * u_scale + vec2(-1.0, 1.0), 0.0, 1.0);
    v_local = a_local;
    v_shape = a_shape;
    v_fill = vec4(a_fill.rgb * a_fill.a, a_fill.a);
    v_line = vec4(a_line.rgb * a_line.a, a_line.a);
    v_kind = int(a_kind + 0.5);
}
"#;

const FS: &str = r#"#version 300 es
precision highp float;
in vec2 v_local;
in vec4 v_shape;
in vec4 v_fill;
in vec4 v_line;
flat in int v_kind;
uniform sampler2D u_tex;
uniform vec2 u_texsize;
out vec4 o_color;

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}
float sdEllipse(vec2 p, vec2 ab) {
    float k0 = length(p / ab);
    float k1 = length(p / (ab * ab));
    return k0 * (k0 - 1.0) / max(k1, 1e-5);
}

void main() {
    if (v_kind == 3) { o_color = v_fill; return; }
    float d;
    float aa;
    if (v_kind == 2) {
        float s = texture(u_tex, v_local).r;
        d = (0.5 - s) * 16.0;               // distance in atlas texels (spread 8)
        aa = max(length(fwidth(v_local * u_texsize)) * 0.75, 0.05);
        d -= v_shape.w;                     // text outlines grow outward, keeping glyphs bold
    } else if (v_kind == 5) {
        float t = clamp(length(v_local / v_shape.xy), 0.0, 1.0);
        float g = (1.0 - t);
        o_color = v_fill * (g * g);
        return;
    } else {
        if (v_kind == 1) d = sdEllipse(v_local, v_shape.xy);
        else d = sdRoundBox(v_local, v_shape.xy, min(v_shape.z, min(v_shape.x, v_shape.y)));
        if (v_kind == 4) {
            // pie: keep the clockwise sector from 12 o'clock covering fraction v_shape.z
            float ang = atan(v_local.x, -v_local.y);
            if (ang < 0.0) ang += 6.2831853;
            float frac = v_shape.z;
            d = sdEllipse(v_local, v_shape.xy);
            float edge = (ang - frac * 6.2831853) * length(v_local);
            if (frac < 1.0) d = max(d, edge);
        }
        aa = max(length(fwidth(v_local)) * 0.75, 1e-4);
    }
    float ow = v_shape.w;
    vec4 col = v_fill;
    if (ow > 0.0) {
        float t = smoothstep(-ow - aa, -ow + aa, d);
        col = mix(v_fill, v_line, t);
    }
    float cover = 1.0 - smoothstep(-aa, aa, d);
    o_color = col * cover;
}
"#;

unsafe fn compile(kind: GLenum, src: &str) -> Result<GLuint, String> {
    let s = glCreateShader(kind);
    let c = CString::new(src).unwrap();
    let p = c.as_ptr();
    glShaderSource(s, 1, &p, std::ptr::null());
    glCompileShader(s);
    let mut ok = 0;
    glGetShaderiv(s, COMPILE_STATUS, &mut ok);
    if ok == 0 {
        let mut buf = vec![0u8; 4096];
        let mut len = 0;
        glGetShaderInfoLog(s, 4096, &mut len, buf.as_mut_ptr() as *mut _);
        return Err(String::from_utf8_lossy(&buf[..len.max(0) as usize]).into_owned());
    }
    Ok(s)
}

impl Font {
    /// Builds the font from Kotlin's ALPHA_8 coverage atlas, converting it to a distance field.
    pub fn from_coverage(cov: &[u8], w: usize, h: usize, glyph_data: &[f32], px: f32) -> Font {
        let mut glyphs = Vec::with_capacity(95);
        for g in glyph_data.chunks_exact(7) {
            glyphs.push(Glyph { x: g[0], y: g[1], w: g[2], h: g[3], bx: g[4], by: g[5], adv: g[6] });
        }
        let sdf = crate::sdf::coverage_to_sdf(cov, w, h, SDF_SPREAD);
        Font { glyphs, px, tex_w: w, tex_h: h, sdf }
    }

    fn glyph(&self, ch: char) -> Option<&Glyph> {
        let c = ch as u32;
        if (32..=126).contains(&c) {
            self.glyphs.get((c - 32) as usize)
        } else {
            self.glyphs.get(('?' as u32 - 32) as usize)
        }
    }
}

impl Renderer {
    pub fn new(font: Font) -> Renderer {
        Renderer {
            verts: Vec::with_capacity(MAX_QUADS * 4),
            gl: None,
            stack: Vec::new(),
            m: Aff::ID,
            alpha: 1.0,
            alpha_stack: Vec::new(),
            font,
            w: 1280.0,
            h: VIRTUAL_H,
            px_w: 1280.0,
            px_h: 720.0,
            draw_calls: 0,
        }
    }

    /// (Re)creates every GL object. Called on each new EGL context.
    pub fn init_gl(&mut self) -> Result<(), String> {
        unsafe {
            let vs = compile(VERTEX_SHADER, VS)?;
            let fs = compile(FRAGMENT_SHADER, FS)?;
            let prog = glCreateProgram();
            glAttachShader(prog, vs);
            glAttachShader(prog, fs);
            glLinkProgram(prog);
            let mut ok = 0;
            glGetProgramiv(prog, LINK_STATUS, &mut ok);
            if ok == 0 {
                let mut buf = vec![0u8; 4096];
                let mut len = 0;
                glGetProgramInfoLog(prog, 4096, &mut len, buf.as_mut_ptr() as *mut _);
                return Err(String::from_utf8_lossy(&buf[..len.max(0) as usize]).into_owned());
            }
            glDeleteShader(vs);
            glDeleteShader(fs);
            let uname = |n: &str| {
                let c = CString::new(n).unwrap();
                glGetUniformLocation(prog, c.as_ptr())
            };
            let u_scale = uname("u_scale");
            let u_tex = uname("u_tex");
            let u_texsize = uname("u_texsize");

            let mut vao = 0;
            glGenVertexArrays(1, &mut vao);
            glBindVertexArray(vao);
            let mut bufs = [0u32; 2];
            glGenBuffers(2, bufs.as_mut_ptr());
            let (vbo, ibo) = (bufs[0], bufs[1]);
            glBindBuffer(ARRAY_BUFFER, vbo);
            glBufferData(ARRAY_BUFFER, (MAX_QUADS * 4 * std::mem::size_of::<Vertex>()) as isize, std::ptr::null(), STREAM_DRAW);
            let stride = std::mem::size_of::<Vertex>() as i32;
            let attr = |i: u32, n: i32, t: GLenum, norm: bool, off: usize| {
                glEnableVertexAttribArray(i);
                glVertexAttribPointer(i, n, t, if norm { TRUE } else { FALSE }, stride, off as *const _);
            };
            attr(0, 2, FLOAT, false, 0);
            attr(1, 2, FLOAT, false, 8);
            attr(2, 4, FLOAT, false, 16);
            attr(3, 4, UNSIGNED_BYTE, true, 32);
            attr(4, 4, UNSIGNED_BYTE, true, 36);
            attr(5, 1, FLOAT, false, 40);

            let mut idx: Vec<u16> = Vec::with_capacity(MAX_QUADS * 6);
            for q in 0..MAX_QUADS as u32 {
                let b = (q * 4) as u16;
                idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
            }
            glBindBuffer(ELEMENT_ARRAY_BUFFER, ibo);
            glBufferData(ELEMENT_ARRAY_BUFFER, (idx.len() * 2) as isize, idx.as_ptr() as *const _, STATIC_DRAW);
            glBindVertexArray(0);

            let mut tex = 0;
            glGenTextures(1, &mut tex);
            glBindTexture(TEXTURE_2D, tex);
            glPixelStorei(UNPACK_ALIGNMENT, 1);
            glTexImage2D(
                TEXTURE_2D,
                0,
                R8,
                self.font.tex_w as i32,
                self.font.tex_h as i32,
                0,
                RED,
                UNSIGNED_BYTE,
                self.font.sdf.as_ptr() as *const _,
            );
            glTexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR);
            glTexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR);
            glTexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_EDGE);
            glTexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_EDGE);

            self.gl = Some(GlState { prog, vao, vbo, ibo, tex, u_scale, u_tex, u_texsize });
        }
        Ok(())
    }

    pub fn resize(&mut self, px_w: i32, px_h: i32) {
        self.px_w = px_w.max(1) as f32;
        self.px_h = px_h.max(1) as f32;
        self.h = VIRTUAL_H;
        self.w = VIRTUAL_H * self.px_w / self.px_h;
        unsafe { glViewport(0, 0, px_w, px_h) };
    }

    /// Converts physical touch pixels into virtual canvas units.
    pub fn to_virtual(&self, x: f32, y: f32) -> V2 {
        v2(x * self.w / self.px_w, y * self.h / self.px_h)
    }
    pub fn px_to_units(&self, px: f32) -> f32 {
        px * self.h / self.px_h
    }

    pub fn begin(&mut self, clear: Col) {
        self.verts.clear();
        self.stack.clear();
        self.alpha_stack.clear();
        self.m = Aff::ID;
        self.alpha = 1.0;
        self.draw_calls = 0;
        unsafe {
            glDisable(DEPTH_TEST);
            glDisable(CULL_FACE);
            glEnable(BLEND);
            glBlendFunc(ONE, ONE_MINUS_SRC_ALPHA);
            glClearColor(clear.0 as f32 / 255.0, clear.1 as f32 / 255.0, clear.2 as f32 / 255.0, 1.0);
            glClear(COLOR_BUFFER_BIT);
        }
    }

    pub fn end(&mut self) {
        self.flush();
    }

    fn flush(&mut self) {
        if self.verts.is_empty() {
            return;
        }
        let Some(g) = &self.gl else {
            self.verts.clear();
            return;
        };
        unsafe {
            glUseProgram(g.prog);
            glUniform2f(g.u_scale, 2.0 / self.w, -2.0 / self.h);
            glUniform1i(g.u_tex, 0);
            glUniform2f(g.u_texsize, self.font.tex_w as f32, self.font.tex_h as f32);
            glActiveTexture(TEXTURE0);
            glBindTexture(TEXTURE_2D, g.tex);
            glBindVertexArray(g.vao);
            glBindBuffer(ARRAY_BUFFER, g.vbo);
            let bytes = self.verts.len() * std::mem::size_of::<Vertex>();
            glBufferData(ARRAY_BUFFER, (MAX_QUADS * 4 * std::mem::size_of::<Vertex>()) as isize, std::ptr::null(), STREAM_DRAW);
            glBufferSubData(ARRAY_BUFFER, 0, bytes as isize, self.verts.as_ptr() as *const _);
            glDrawElements(TRIANGLES, (self.verts.len() / 4 * 6) as i32, UNSIGNED_SHORT, std::ptr::null());
            glBindVertexArray(0);
        }
        self.draw_calls += 1;
        self.verts.clear();
    }

    // ---------------------------------------------------------------- transforms

    pub fn push(&mut self) {
        self.stack.push(self.m);
        self.alpha_stack.push(self.alpha);
    }
    pub fn pop(&mut self) {
        if let Some(m) = self.stack.pop() {
            self.m = m;
        }
        if let Some(a) = self.alpha_stack.pop() {
            self.alpha = a;
        }
    }
    pub fn translate(&mut self, x: f32, y: f32) {
        self.m = self.m.mul(&Aff { tx: x, ty: y, ..Aff::ID });
    }
    pub fn rotate(&mut self, ang: f32) {
        let (s, c) = ang.sin_cos();
        self.m = self.m.mul(&Aff { a: c, b: s, c: -s, d: c, tx: 0.0, ty: 0.0 });
    }
    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.m = self.m.mul(&Aff { a: sx, d: sy, ..Aff::ID });
    }
    /// Multiplies the opacity of everything drawn until the matching pop().
    pub fn fade(&mut self, a: f32) {
        self.alpha *= a.clamp(0.0, 1.0);
    }

    // ---------------------------------------------------------------- primitives

    fn col(&self, c: Col) -> [u8; 4] {
        [c.0, c.1, c.2, (c.3 as f32 * self.alpha) as u8]
    }

    fn quad(&mut self, corners: [V2; 4], locals: [V2; 4], shape: [f32; 4], fills: [Col; 4], line: Col, kind: f32) {
        if self.verts.len() + 4 > MAX_QUADS * 4 {
            self.flush();
        }
        let ln = self.col(line);
        for i in 0..4 {
            let p = self.m.apply(corners[i]);
            self.verts.push(Vertex {
                pos: [p.x, p.y],
                local: [locals[i].x, locals[i].y],
                shape,
                fill: self.col(fills[i]),
                line: ln,
                kind,
            });
        }
    }

    /// Core shape: centred at (cx, cy), half extents (hw, hh), rotated by `rot`.
    /// `top`/`bottom` give a vertical gradient (in shape space); the outline is drawn inside the edge.
    fn shape(&mut self, cx: f32, cy: f32, hw: f32, hh: f32, rot: f32, radius: f32, lw: f32, top: Col, bottom: Col, line: Col, kind: f32) {
        if hw <= 0.0 || hh <= 0.0 {
            return;
        }
        let m = 2.0;
        let ex = hw + m;
        let ey = hh + m;
        let locals = [v2(-ex, -ey), v2(ex, -ey), v2(ex, ey), v2(-ex, ey)];
        let (s, c) = rot.sin_cos();
        let mut corners = locals;
        for p in corners.iter_mut() {
            *p = v2(cx + p.x * c - p.y * s, cy + p.x * s + p.y * c);
        }
        let fills = [top, top, bottom, bottom];
        self.quad(corners, locals, [hw, hh, radius, lw], fills, line, kind);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Col) {
        let corners = [v2(x, y), v2(x + w, y), v2(x + w, y + h), v2(x, y + h)];
        self.quad(corners, corners, [0.0; 4], [c; 4], c, K_SOLID);
    }
    pub fn grad_rect(&mut self, x: f32, y: f32, w: f32, h: f32, top: Col, bottom: Col) {
        let corners = [v2(x, y), v2(x + w, y), v2(x + w, y + h), v2(x, y + h)];
        self.quad(corners, corners, [0.0; 4], [top, top, bottom, bottom], top, K_SOLID);
    }
    pub fn hgrad_rect(&mut self, x: f32, y: f32, w: f32, h: f32, left: Col, right: Col) {
        let corners = [v2(x, y), v2(x + w, y), v2(x + w, y + h), v2(x, y + h)];
        self.quad(corners, corners, [0.0; 4], [left, right, right, left], left, K_SOLID);
    }
    pub fn tri(&mut self, a: V2, b: V2, c: V2, col: Col) {
        self.quad([a, b, c, c], [a, b, c, c], [0.0; 4], [col; 4], col, K_SOLID);
    }

    /// Rounded rectangle centred at (cx, cy) with full size (w, h).
    pub fn rrect(&mut self, cx: f32, cy: f32, w: f32, h: f32, r: f32, fill: Col, line: Col, lw: f32) {
        self.shape(cx, cy, w * 0.5, h * 0.5, 0.0, r, lw, fill, fill, line, K_RRECT);
    }
    pub fn rrect_rot(&mut self, cx: f32, cy: f32, w: f32, h: f32, r: f32, rot: f32, fill: Col, line: Col, lw: f32) {
        self.shape(cx, cy, w * 0.5, h * 0.5, rot, r, lw, fill, fill, line, K_RRECT);
    }
    pub fn rrect_grad(&mut self, cx: f32, cy: f32, w: f32, h: f32, r: f32, top: Col, bottom: Col, line: Col, lw: f32) {
        self.shape(cx, cy, w * 0.5, h * 0.5, 0.0, r, lw, top, bottom, line, K_RRECT);
    }
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, fill: Col, line: Col, lw: f32) {
        self.shape(cx, cy, rx, ry, 0.0, 0.0, lw, fill, fill, line, K_ELLIPSE);
    }
    pub fn ellipse_rot(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, fill: Col, line: Col, lw: f32) {
        self.shape(cx, cy, rx, ry, rot, 0.0, lw, fill, fill, line, K_ELLIPSE);
    }
    pub fn ellipse_grad(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, top: Col, bottom: Col, line: Col, lw: f32) {
        self.shape(cx, cy, rx, ry, 0.0, 0.0, lw, top, bottom, line, K_ELLIPSE);
    }
    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, fill: Col, line: Col, lw: f32) {
        self.shape(cx, cy, r, r, 0.0, r, lw, fill, fill, line, K_RRECT);
    }
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, fill: Col) {
        self.shape(cx, cy, r, r, 0.0, r, 0.0, fill, fill, fill, K_RRECT);
    }
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, lw: f32, line: Col) {
        self.shape(cx, cy, r, r, 0.0, r, lw, Col::CLEAR, Col::CLEAR, line, K_RRECT);
    }
    /// Thick line with round caps.
    pub fn line(&mut self, a: V2, b: V2, width: f32, fill: Col) {
        let d = b - a;
        let len = d.len();
        let mid = a.lerp(b, 0.5);
        let ang = d.y.atan2(d.x);
        self.shape(mid.x, mid.y, len * 0.5 + width * 0.5, width * 0.5, ang, width * 0.5, 0.0, fill, fill, fill, K_RRECT);
    }
    pub fn line_outlined(&mut self, a: V2, b: V2, width: f32, fill: Col, line: Col, lw: f32) {
        let d = b - a;
        let len = d.len();
        let mid = a.lerp(b, 0.5);
        let ang = d.y.atan2(d.x);
        self.shape(mid.x, mid.y, len * 0.5 + width * 0.5, width * 0.5, ang, width * 0.5, lw, fill, fill, line, K_RRECT);
    }
    /// Clockwise pie from 12 o'clock covering `frac` of the circle.
    pub fn pie(&mut self, cx: f32, cy: f32, r: f32, frac: f32, fill: Col) {
        if frac <= 0.0 {
            return;
        }
        self.shape(cx, cy, r, r, 0.0, frac.min(1.0), 0.0, fill, fill, fill, K_PIE);
    }
    /// Soft radial glow (quadratic falloff), great for particles and highlights.
    pub fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Col) {
        let locals = [v2(-r, -r), v2(r, -r), v2(r, r), v2(-r, r)];
        let corners = [v2(cx - r, cy - r), v2(cx + r, cy - r), v2(cx + r, cy + r), v2(cx - r, cy + r)];
        self.quad(corners, locals, [r, r, 0.0, 0.0], [c; 4], c, K_GLOW);
    }
    /// Five-pointed star built from triangles.
    pub fn star(&mut self, cx: f32, cy: f32, r: f32, rot: f32, fill: Col, line: Col, lw: f32) {
        let pts = |rr: f32, inner: f32| -> [V2; 10] {
            let mut p = [V2::default(); 10];
            for (i, q) in p.iter_mut().enumerate() {
                let a = rot - std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let rad = if i % 2 == 0 { rr } else { rr * inner };
                *q = v2(cx + a.cos() * rad, cy + a.sin() * rad);
            }
            p
        };
        let c = v2(cx, cy);
        if lw > 0.0 {
            let o = pts(r, 0.5);
            for i in 0..10 {
                self.tri(c, o[i], o[(i + 1) % 10], line);
            }
            for i in 0..5 {
                let a = rot - std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
                self.disc(cx + a.cos() * (r - lw * 0.1), cy + a.sin() * (r - lw * 0.1), lw * 0.75, line);
            }
        }
        let p = pts(r - lw, 0.5);
        for i in 0..10 {
            self.tri(c, p[i], p[(i + 1) % 10], fill);
        }
    }

    // ---------------------------------------------------------------- text

    pub fn text_width(&self, s: &str, size: f32) -> f32 {
        let scale = size / self.font.px;
        s.lines()
            .map(|l| l.chars().map(|c| self.font.glyph(c).map(|g| g.adv).unwrap_or(0.0)).sum::<f32>() * scale)
            .fold(0.0, f32::max)
    }

    /// Draws text centred vertically on `y`. `outline` is in font texels (0 = none, up to ~6).
    pub fn text_ex(&mut self, s: &str, x: f32, y: f32, size: f32, align: Align, fill: Col, line: Col, outline: f32) {
        let scale = size / self.font.px;
        let line_h = size * 1.12;
        let lines: Vec<&str> = s.split('\n').collect();
        let total = line_h * lines.len() as f32;
        let cap = self.font.px * 0.71 * scale;
        let mut base_y = y - total * 0.5 + line_h * 0.5 + cap * 0.5;
        let tw = self.font.tex_w as f32;
        let th = self.font.tex_h as f32;
        for l in lines {
            let w: f32 = l.chars().map(|c| self.font.glyph(c).map(|g| g.adv).unwrap_or(0.0)).sum::<f32>() * scale;
            let mut pen = match align {
                Align::Left => x,
                Align::Center => x - w * 0.5,
                Align::Right => x - w,
            };
            for ch in l.chars() {
                let Some(g) = self.font.glyph(ch).copied() else { continue };
                if ch != ' ' {
                    let x0 = pen + g.bx * scale;
                    let y0 = base_y + g.by * scale;
                    let x1 = x0 + g.w * scale;
                    let y1 = y0 + g.h * scale;
                    let u0 = g.x / tw;
                    let v0 = g.y / th;
                    let u1 = (g.x + g.w) / tw;
                    let v1 = (g.y + g.h) / th;
                    self.quad(
                        [v2(x0, y0), v2(x1, y0), v2(x1, y1), v2(x0, y1)],
                        [v2(u0, v0), v2(u1, v0), v2(u1, v1), v2(u0, v1)],
                        [0.0, 0.0, 0.0, outline],
                        [fill; 4],
                        line,
                        K_TEXT,
                    );
                }
                pen += g.adv * scale;
            }
            base_y += line_h;
        }
    }

    pub fn text(&mut self, s: &str, x: f32, y: f32, size: f32, align: Align, fill: Col) {
        self.text_ex(s, x, y, size, align, fill, fill, 0.0);
    }

    /// Cartoon title text: dark outline plus a drop shadow.
    pub fn text_bold(&mut self, s: &str, x: f32, y: f32, size: f32, align: Align, fill: Col, outline: Col) {
        let sh = size * 0.06;
        self.text_ex(s, x, y + sh, size, align, outline.a(0.45), outline.a(0.45), 5.0);
        self.text_ex(s, x, y, size, align, fill, outline, 5.0);
    }
}
