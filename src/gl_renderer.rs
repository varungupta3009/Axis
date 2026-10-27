use crate::ffi::*;
use libc::{c_int, c_void};
use std::collections::HashMap;
use std::ffi::CString;
use std::path::Path;
use std::ptr;

const RECT_VS: &str = r#"
attribute vec2 a_unit;
uniform vec2 u_res;
uniform vec4 u_quad;
varying vec2 v_pos;
void main() {
    v_pos = u_quad.xy + a_unit * u_quad.zw;
    vec2 ndc = (v_pos / u_res) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
}
"#;

const RECT_FS: &str = r#"
precision mediump float;
varying vec2 v_pos;
uniform vec4 u_rect;
uniform float u_radius;
uniform vec4 u_fill_top;
uniform vec4 u_fill_bot;
uniform vec4 u_border_color;
uniform float u_border_width;
uniform float u_glow_radius;
uniform vec4 u_glow_color;
uniform float u_dashed;

float sdRoundedBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

void main() {
    vec2 center = u_rect.xy + u_rect.zw * 0.5;
    vec2 half_size = u_rect.zw * 0.5;
    vec2 p = v_pos - center;
    float r = min(u_radius, min(half_size.x, half_size.y));
    float d = sdRoundedBox(p, half_size, r);

    // 160-degree linear gradient parameter
    vec2 uv = (v_pos - u_rect.xy) / max(u_rect.zw, vec2(1.0));
    float grad_t = clamp(uv.x * 0.34 + uv.y * 0.94, 0.0, 1.0);
    vec4 fill = mix(u_fill_top, u_fill_bot, grad_t);

    float inside_alpha = 1.0 - smoothstep(-0.6, 0.6, d);
    vec4 col = vec4(fill.rgb, fill.a * inside_alpha);

    if (u_glow_radius > 0.5 && d > -1.0) {
        float g = exp(-max(d, 0.0) / (u_glow_radius * 0.42));
        float glow_a = u_glow_color.a * g * (1.0 - inside_alpha * 0.65);
        col = mix(vec4(u_glow_color.rgb, glow_a), col, inside_alpha);
    }

    if (u_border_width > 0.1) {
        float b_outer = 1.0 - smoothstep(-0.6, 0.6, d);
        float b_inner = 1.0 - smoothstep(-0.6, 0.6, d + u_border_width);
        float b_mask = clamp(b_outer - b_inner, 0.0, 1.0);
        if (u_dashed > 0.5) {
            float perim = mod(v_pos.x + v_pos.y, 18.0);
            if (perim > 10.0) b_mask = 0.0;
        }
        col = mix(col, vec4(u_border_color.rgb, u_border_color.a), b_mask * u_border_color.a);
    }

    if (col.a < 0.003) discard;
    gl_FragColor = col;
}
"#;

const TEX_VS: &str = r#"
attribute vec2 a_unit;
uniform vec2 u_res;
uniform vec4 u_quad;
varying vec2 v_uv;
varying vec2 v_pos;
void main() {
    v_uv = a_unit;
    v_pos = u_quad.xy + a_unit * u_quad.zw;
    vec2 ndc = (v_pos / u_res) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
}
"#;

const TEX_FS: &str = r#"
precision mediump float;
varying vec2 v_uv;
varying vec2 v_pos;
uniform vec4 u_rect;
uniform float u_radius;
uniform vec4 u_color;
uniform sampler2D u_tex;

float sdRoundedBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

void main() {
    vec4 tex_col = texture2D(u_tex, v_uv);
    float mask = 1.0;
    if (u_radius > 0.5) {
        vec2 center = u_rect.xy + u_rect.zw * 0.5;
        vec2 half_size = u_rect.zw * 0.5;
        float d = sdRoundedBox(v_pos - center, half_size, u_radius);
        mask = 1.0 - smoothstep(-0.7, 0.7, d);
    }
    vec4 out_col = tex_col * u_color;
    out_col.a *= mask;
    if (out_col.a < 0.004) discard;
    gl_FragColor = out_col;
}
"#;

const PRIM_VS: &str = r#"
attribute vec2 a_pos;
attribute vec4 a_col;
uniform vec2 u_res;
varying vec4 v_col;
void main() {
    v_col = a_col;
    vec2 ndc = (a_pos / u_res) * 2.0 - 1.0;
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
}
"#;

const PRIM_FS: &str = r#"
precision mediump float;
varying vec4 v_col;
void main() {
    gl_FragColor = v_col;
}
"#;

#[derive(Clone, Copy)]
pub struct TextureInfo {
    pub id: GLuint,
    pub w: i32,
    pub h: i32,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct TextKey {
    size: u16,
    bold: bool,
    text: String,
}

#[derive(Clone, Copy)]
pub struct Star {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub ph: f32,
    pub spd: f32,
    pub base: f32,
}

#[derive(Clone, Copy)]
pub struct NodePt {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub r: f32,
    pub ph: f32,
}

pub struct PlexusLayer {
    pub rgb: [f32; 3],
    pub core: [f32; 3],
    pub link: f32,
    pub dot_a: f32,
    pub line_a: f32,
    pub halo: f32,
    pub pts: Vec<NodePt>,
}

#[derive(Clone, Copy)]
pub struct TrailPt {
    pub x: f32,
    pub y: f32,
    pub life: f32,
}

pub struct ConstellationsState {
    pub key: String,
    pub stars: Vec<Star>,
    pub layers: Vec<PlexusLayer>,
    pub trail: Vec<TrailPt>,
    pub trail_rgb: [f32; 3],
    pub mx: f32,
    pub my: f32,
    pub last_move_ms: u32,
    pub rng_state: u64,
}

impl ConstellationsState {
    pub fn new() -> Self {
        Self {
            key: String::new(),
            stars: Vec::new(),
            layers: Vec::new(),
            trail: Vec::with_capacity(96),
            trail_rgb: [1.0, 0.196, 0.196],
            mx: -9999.0,
            my: -9999.0,
            last_move_ms: 0,
            rng_state: 0x853c_49e6_748f_ea9b,
        }
    }

    fn rand_f32(&mut self) -> f32 {
        // xorshift64*
        let mut x = self.rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng_state = x;
        let v = (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as u32;
        (v as f32) / ((1u32 << 24) as f32)
    }

    pub fn ensure_config(&mut self, density_str: &str, speed_str: &str, theme: &str) {
        let new_key = format!("{}|{}|{}", density_str, speed_str, theme);
        if self.key == new_key && !self.layers.is_empty() {
            return;
        }
        self.key = new_key;
        let density: f32 = match density_str {
            "low" => 0.6,
            "high" => 1.45,
            _ => 1.0,
        };
        let speed_mul: f32 = match speed_str {
            "slow" => 0.55,
            "fast" => 1.65,
            _ => 1.0,
        };

        let (layer_colors, trail_rgb): ([([u8; 3], [u8; 3]); 3], [u8; 3]) = match theme {
            "mono" => (
                [
                    ([70, 120, 200], [120, 170, 240]),
                    ([100, 150, 230], [160, 200, 255]),
                    ([185, 210, 245], [235, 244, 255]),
                ],
                [120, 170, 255],
            ),
            "ember" => (
                [
                    ([190, 105, 55], [235, 165, 105]),
                    ([215, 75, 55], [255, 140, 110]),
                    ([242, 212, 170], [255, 242, 222]),
                ],
                [255, 120, 40],
            ),
            "aurora" => (
                [
                    ([55, 155, 135], [115, 215, 185]),
                    ([85, 110, 215], [145, 170, 255]),
                    ([190, 240, 220], [240, 255, 250]),
                ],
                [80, 230, 160],
            ),
            _ => (
                [
                    ([95, 150, 205], [150, 197, 240]),
                    ([206, 92, 92], [242, 156, 156]),
                    ([216, 226, 238], [255, 255, 255]),
                ],
                [255, 50, 50],
            ),
        };

        self.trail_rgb = [
            trail_rgb[0] as f32 / 255.0,
            trail_rgb[1] as f32 / 255.0,
            trail_rgb[2] as f32 / 255.0,
        ];

        let w = 1920.0;
        let h = 1080.0;
        let tau = std::f32::consts::TAU;

        let star_count = (300.0 * density).round() as usize;
        self.stars.clear();
        for _ in 0..star_count {
            let x = self.rand_f32() * w;
            let y = self.rand_f32() * h;
            let r = 0.55 + self.rand_f32() * 1.45;
            let ph = self.rand_f32() * tau;
            let spd = 0.001 + self.rand_f32() * 0.004;
            let base = 0.30 + self.rand_f32() * 0.65;
            self.stars.push(Star {
                x,
                y,
                r,
                ph,
                spd,
                base,
            });
        }

        // 3 Depth layers: Background (68), Midground (48), Foreground (34)
        let specs: [(usize, f32, f32, f32, f32, f32, f32, f32); 3] = [
            (68, 0.10, 0.9, 1.8, 150.0, 0.82, 0.42, 0.16),
            (48, 0.17, 1.4, 2.5, 182.0, 0.92, 0.60, 0.20),
            (34, 0.28, 2.1, 3.9, 232.0, 1.00, 0.72, 0.30),
        ];

        self.layers.clear();
        for (idx, (base_cnt, spd, r_min, r_max, link, dot_a, line_a, halo)) in
            specs.iter().enumerate()
        {
            let cnt = ((*base_cnt as f32) * density).round() as usize;
            let mut pts = Vec::with_capacity(cnt);
            for _ in 0..cnt {
                let a = self.rand_f32() * tau;
                let v = *spd * speed_mul;
                let x = self.rand_f32() * w;
                let y = self.rand_f32() * h;
                let r = *r_min + self.rand_f32() * (*r_max - *r_min);
                let ph = self.rand_f32() * tau;
                pts.push(NodePt {
                    x,
                    y,
                    vx: a.cos() * v,
                    vy: a.sin() * v,
                    r,
                    ph,
                });
            }
            let (rgb_u8, core_u8) = layer_colors[idx];
            self.layers.push(PlexusLayer {
                rgb: [
                    rgb_u8[0] as f32 / 255.0,
                    rgb_u8[1] as f32 / 255.0,
                    rgb_u8[2] as f32 / 255.0,
                ],
                core: [
                    core_u8[0] as f32 / 255.0,
                    core_u8[1] as f32 / 255.0,
                    core_u8[2] as f32 / 255.0,
                ],
                link: *link,
                dot_a: *dot_a,
                line_a: *line_a,
                halo: *halo,
                pts,
            });
        }
    }

    pub fn on_pointer_move(&mut self, x: f32, y: f32, now_ms: u32) {
        self.mx = x;
        self.my = y;
        self.last_move_ms = now_ms;
        self.trail.push(TrailPt { x, y, life: 1.0 });
        if self.trail.len() > 80 {
            self.trail.remove(0);
        }
    }

    pub fn step_and_draw(
        &mut self,
        r: &mut GlRenderer,
        now_ms: u32,
        dt_frames: f32,
        animated: bool,
        alpha_mul: f32,
    ) {
        if alpha_mul <= 0.01 {
            return;
        }
        let t = now_ms as f32;
        let w = 1920.0;
        let h = 1080.0;
        let hover_r = 300.0;
        let hover_r2 = hover_r * hover_r;
        let age = now_ms.saturating_sub(self.last_move_ms) as f32;
        let p_on = self.last_move_ms > 0 && age < 1400.0;
        let p_fade = if p_on {
            (1.0 - age / 1400.0).max(0.0)
        } else {
            0.0
        };

        r.tri_verts.clear();
        r.line_verts.clear();

        // 1. Twinkling starfield
        for s in &self.stars {
            let tw = if animated {
                0.35 + 0.65 * (0.5 + 0.5 * (t * s.spd + s.ph).sin())
            } else {
                0.75
            };
            let a = s.base * tw * alpha_mul;
            r.push_diamond(s.x, s.y, s.r * 1.25, [1.0, 1.0, 1.0, a]);
        }

        // 2. Depth layers back -> front
        for layer in &mut self.layers {
            if animated {
                for p in &mut layer.pts {
                    p.x += p.vx * dt_frames;
                    p.y += p.vy * dt_frames;
                    if p_on {
                        let dx = self.mx - p.x;
                        let dy = self.my - p.y;
                        let d2 = dx * dx + dy * dy;
                        if d2 < hover_r2 {
                            let f = (1.0 - d2.sqrt() / hover_r) * 0.05 * p_fade;
                            p.x += dx * f;
                            p.y += dy * f;
                        }
                    }
                    if p.x < 0.0 {
                        p.x += w;
                    } else if p.x > w {
                        p.x -= w;
                    }
                    if p.y < 0.0 {
                        p.y += h;
                    } else if p.y > h {
                        p.y -= h;
                    }
                }
            }

            let l2 = layer.link * layer.link;
            let n = layer.pts.len();
            for i in 0..n {
                let pa = layer.pts[i];
                for j in (i + 1)..n {
                    let pb = layer.pts[j];
                    let dx = pa.x - pb.x;
                    let dy = pa.y - pb.y;
                    let d2 = dx * dx + dy * dy;
                    if d2 < l2 {
                        let a = (1.0 - d2.sqrt() / layer.link) * layer.line_a * alpha_mul;
                        let col = [layer.rgb[0], layer.rgb[1], layer.rgb[2], a];
                        r.push_line(pa.x, pa.y, pb.x, pb.y, col);
                    }
                }
            }

            for p in &layer.pts {
                let tw = if animated {
                    0.72 + 0.28 * (t * 0.002 + p.ph).sin()
                } else {
                    0.88
                };
                let halo_a = layer.dot_a * tw * layer.halo * alpha_mul;
                let core_a = layer.dot_a * tw * alpha_mul;
                r.push_octagon(
                    p.x,
                    p.y,
                    p.r * 2.8,
                    [layer.rgb[0], layer.rgb[1], layer.rgb[2], halo_a],
                );
                r.push_octagon(
                    p.x,
                    p.y,
                    p.r * 1.15,
                    [layer.core[0], layer.core[1], layer.core[2], core_a],
                );
            }
        }

        // 3. Magic Remote hover network & cursor node
        if p_on {
            for layer in &self.layers {
                for p in &layer.pts {
                    let dx = self.mx - p.x;
                    let dy = self.my - p.y;
                    let d2 = dx * dx + dy * dy;
                    if d2 < hover_r2 {
                        let a = (1.0 - d2.sqrt() / hover_r) * 0.5 * p_fade * alpha_mul;
                        r.push_line(
                            self.mx,
                            self.my,
                            p.x,
                            p.y,
                            [0.745, 0.843, 0.941, a],
                        );
                    }
                }
            }
            r.push_octagon(
                self.mx,
                self.my,
                4.2,
                [0.90, 0.94, 0.99, 0.95 * p_fade * alpha_mul],
            );
        }

        // 4. Pointer trail
        if !self.trail.is_empty() {
            for i in (0..self.trail.len()).rev() {
                if animated {
                    self.trail[i].life -= 0.03 * dt_frames;
                }
                if self.trail[i].life <= 0.0 {
                    self.trail.remove(i);
                } else {
                    let q = self.trail[i];
                    let rad = 3.2 * q.life + 1.0;
                    let a = q.life * 0.7 * alpha_mul;
                    r.push_octagon(
                        q.x,
                        q.y,
                        rad,
                        [self.trail_rgb[0], self.trail_rgb[1], self.trail_rgb[2], a],
                    );
                }
            }
        }

        r.flush_primitives(1.35);
    }
}

pub struct GlRenderer {
    pub width: f32,
    pub height: f32,
    rect_prog: GLuint,
    rect_a_unit: GLint,
    rect_u_res: GLint,
    rect_u_quad: GLint,
    rect_u_rect: GLint,
    rect_u_radius: GLint,
    rect_u_fill_top: GLint,
    rect_u_fill_bot: GLint,
    rect_u_border_color: GLint,
    rect_u_border_width: GLint,
    rect_u_glow_radius: GLint,
    rect_u_glow_color: GLint,
    rect_u_dashed: GLint,

    tex_prog: GLuint,
    tex_a_unit: GLint,
    tex_u_res: GLint,
    tex_u_quad: GLint,
    tex_u_rect: GLint,
    tex_u_radius: GLint,
    tex_u_color: GLint,
    tex_u_tex: GLint,

    prim_prog: GLuint,
    prim_a_pos: GLint,
    prim_a_col: GLint,
    prim_u_res: GLint,

    fonts_reg: HashMap<u16, *mut TTF_Font>,
    fonts_bold: HashMap<u16, *mut TTF_Font>,
    font_family: String,
    reg_font_path: String,
    bold_font_path: String,
    text_cache: HashMap<TextKey, TextureInfo>,
    icon_cache: HashMap<String, Option<TextureInfo>>,

    pub tri_verts: Vec<f32>,
    pub line_verts: Vec<f32>,
}

fn compile_shader(src: &str, type_: GLenum) -> GLuint {
    unsafe {
        let s = glCreateShader(type_);
        let c_str = CString::new(src).unwrap();
        let ptr = c_str.as_ptr();
        glShaderSource(s, 1, &ptr, ptr::null());
        glCompileShader(s);
        let mut status: GLint = 0;
        glGetShaderiv(s, GL_COMPILE_STATUS, &mut status);
        s
    }
}

fn link_program(vs_src: &str, fs_src: &str) -> GLuint {
    unsafe {
        let vs = compile_shader(vs_src, GL_VERTEX_SHADER);
        let fs = compile_shader(fs_src, GL_FRAGMENT_SHADER);
        let prog = glCreateProgram();
        glAttachShader(prog, vs);
        glAttachShader(prog, fs);
        glLinkProgram(prog);
        let mut link_status: GLint = 0;
        glGetProgramiv(prog, GL_LINK_STATUS, &mut link_status);
        glDeleteShader(vs);
        glDeleteShader(fs);
        prog
    }
}

fn loc_attr(prog: GLuint, name: &str) -> GLint {
    let c = CString::new(name).unwrap();
    unsafe { glGetAttribLocation(prog, c.as_ptr()) }
}

fn loc_unif(prog: GLuint, name: &str) -> GLint {
    let c = CString::new(name).unwrap();
    unsafe { glGetUniformLocation(prog, c.as_ptr()) }
}

impl GlRenderer {
    pub fn new(base_dir: &Path, width: f32, height: f32) -> Self {
        let rect_prog = link_program(RECT_VS, RECT_FS);
        let tex_prog = link_program(TEX_VS, TEX_FS);
        let prim_prog = link_program(PRIM_VS, PRIM_FS);

        unsafe {
            glEnable(GL_BLEND);
            glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
            glPixelStorei(GL_UNPACK_ALIGNMENT, 4);
        }

        let sys_reg = Path::new("/usr/share/fonts/LG_Smart_UI-Regular.ttf");
        let reg_font_path = if sys_reg.exists() {
            sys_reg.to_string_lossy().to_string()
        } else {
            base_dir
                .join("assets/fonts/regular.ttf")
                .to_string_lossy()
                .to_string()
        };
        let sys_bold = Path::new("/usr/share/fonts/LG_Smart_UI-SemiBold.ttf");
        let bold_font_path = if sys_bold.exists() {
            sys_bold.to_string_lossy().to_string()
        } else {
            base_dir
                .join("assets/fonts/bold.ttf")
                .to_string_lossy()
                .to_string()
        };

        Self {
            width,
            height,
            rect_prog,
            rect_a_unit: loc_attr(rect_prog, "a_unit"),
            rect_u_res: loc_unif(rect_prog, "u_res"),
            rect_u_quad: loc_unif(rect_prog, "u_quad"),
            rect_u_rect: loc_unif(rect_prog, "u_rect"),
            rect_u_radius: loc_unif(rect_prog, "u_radius"),
            rect_u_fill_top: loc_unif(rect_prog, "u_fill_top"),
            rect_u_fill_bot: loc_unif(rect_prog, "u_fill_bot"),
            rect_u_border_color: loc_unif(rect_prog, "u_border_color"),
            rect_u_border_width: loc_unif(rect_prog, "u_border_width"),
            rect_u_glow_radius: loc_unif(rect_prog, "u_glow_radius"),
            rect_u_glow_color: loc_unif(rect_prog, "u_glow_color"),
            rect_u_dashed: loc_unif(rect_prog, "u_dashed"),

            tex_prog,
            tex_a_unit: loc_attr(tex_prog, "a_unit"),
            tex_u_res: loc_unif(tex_prog, "u_res"),
            tex_u_quad: loc_unif(tex_prog, "u_quad"),
            tex_u_rect: loc_unif(tex_prog, "u_rect"),
            tex_u_radius: loc_unif(tex_prog, "u_radius"),
            tex_u_color: loc_unif(tex_prog, "u_color"),
            tex_u_tex: loc_unif(tex_prog, "u_tex"),

            prim_prog,
            prim_a_pos: loc_attr(prim_prog, "a_pos"),
            prim_a_col: loc_attr(prim_prog, "a_col"),
            prim_u_res: loc_unif(prim_prog, "u_res"),

            fonts_reg: HashMap::new(),
            fonts_bold: HashMap::new(),
            font_family: "lg".to_string(),
            reg_font_path,
            bold_font_path,
            text_cache: HashMap::new(),
            icon_cache: HashMap::new(),
            tri_verts: Vec::with_capacity(16384),
            line_verts: Vec::with_capacity(16384),
        }
    }

    pub fn set_font_family(&mut self, base_dir: &Path, family: &str) {
        if self.font_family == family {
            return;
        }
        self.font_family = family.to_string();

        unsafe {
            for (_, f) in self.fonts_reg.drain() {
                if !f.is_null() {
                    TTF_CloseFont(f);
                }
            }
            for (_, f) in self.fonts_bold.drain() {
                if !f.is_null() {
                    TTF_CloseFont(f);
                }
            }
            for (_, v) in self.text_cache.drain() {
                glDeleteTextures(1, &v.id);
            }
        }

        if family == "titillium" {
            let tit_reg = base_dir.join("assets/fonts/TitilliumWeb-Regular.ttf");
            let tit_bold = base_dir.join("assets/fonts/TitilliumWeb-SemiBold.ttf");
            if tit_reg.exists() && tit_bold.exists() {
                self.reg_font_path = tit_reg.to_string_lossy().to_string();
                self.bold_font_path = tit_bold.to_string_lossy().to_string();
                return;
            }
        }

        let sys_reg = Path::new("/usr/share/fonts/LG_Smart_UI-Regular.ttf");
        self.reg_font_path = if sys_reg.exists() {
            sys_reg.to_string_lossy().to_string()
        } else {
            base_dir
                .join("assets/fonts/regular.ttf")
                .to_string_lossy()
                .to_string()
        };
        let sys_bold = Path::new("/usr/share/fonts/LG_Smart_UI-SemiBold.ttf");
        self.bold_font_path = if sys_bold.exists() {
            sys_bold.to_string_lossy().to_string()
        } else {
            base_dir
                .join("assets/fonts/bold.ttf")
                .to_string_lossy()
                .to_string()
        };
    }

    pub fn get_font(&mut self, size: u16, bold: bool) -> *mut TTF_Font {
        let map = if bold {
            &mut self.fonts_bold
        } else {
            &mut self.fonts_reg
        };
        if let Some(&f) = map.get(&size) {
            return f;
        }
        let path = if bold {
            &self.bold_font_path
        } else {
            &self.reg_font_path
        };
        let c_path = CString::new(path.as_str()).unwrap();
        let font = unsafe {
            let f = TTF_OpenFont(c_path.as_ptr(), size as c_int);
            if !f.is_null() {
                TTF_SetFontHinting(f, TTF_HINTING_NORMAL);
            }
            f
        };
        map.insert(size, font);
        font
    }

    unsafe fn surface_to_texture(raw_surf: *mut SDL_Surface, is_text: bool) -> Option<TextureInfo> {
        if raw_surf.is_null() {
            return None;
        }
        let conv = SDL_ConvertSurfaceFormat(raw_surf, SDL_PIXELFORMAT_ABGR8888, 0);
        SDL_FreeSurface(raw_surf);
        if conv.is_null() {
            return None;
        }
        let w = (*conv).w;
        let h = (*conv).h;
        let pitch = (*conv).pitch;
        let pixels = (*conv).pixels as *const u8;

        let mut tight = vec![0u8; (w * h * 4) as usize];
        for row in 0..h {
            let src_row = pixels.offset((row * pitch) as isize);
            let dst_row = tight.as_mut_ptr().offset((row * w * 4) as isize);
            ptr::copy_nonoverlapping(src_row, dst_row, (w * 4) as usize);
        }
        SDL_FreeSurface(conv);

        if is_text {
            // Eliminate (0,0,0,0) transparent-black edge bleed on white glyph surfaces
            for px in tight.chunks_exact_mut(4) {
                px[0] = 255;
                px[1] = 255;
                px[2] = 255;
            }
        }

        let filter = if is_text { GL_NEAREST } else { GL_LINEAR };
        let mut tex_id: GLuint = 0;
        glGenTextures(1, &mut tex_id);
        glBindTexture(GL_TEXTURE_2D, tex_id);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, filter);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, filter);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            GL_RGBA as GLint,
            w,
            h,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            tight.as_ptr() as *const c_void,
        );

        Some(TextureInfo { id: tex_id, w, h })
    }

    pub fn preload_icon(&mut self, base_dir: &Path, rel_path: &str) -> Option<TextureInfo> {
        if rel_path.is_empty() {
            return None;
        }
        if let Some(&cached) = self.icon_cache.get(rel_path) {
            return cached;
        }
        let mut full_path = base_dir.join(rel_path);
        if !full_path.exists() {
            let alt = base_dir.join("assets").join(rel_path);
            if alt.exists() {
                full_path = alt;
            }
        }
        let c_path = CString::new(full_path.to_string_lossy().as_bytes()).ok()?;
        let info = unsafe {
            let surf = IMG_Load(c_path.as_ptr());
            Self::surface_to_texture(surf, false)
        };
        self.icon_cache.insert(rel_path.to_string(), info);
        info
    }

    pub fn get_text_texture(&mut self, text: &str, size: u16, bold: bool) -> Option<TextureInfo> {
        if text.is_empty() {
            return None;
        }
        let key = TextKey {
            size,
            bold,
            text: text.to_string(),
        };
        if let Some(&info) = self.text_cache.get(&key) {
            return Some(info);
        }
        if self.text_cache.len() > 320 {
            for (_, v) in self.text_cache.drain() {
                unsafe {
                    glDeleteTextures(1, &v.id);
                }
            }
        }
        let font = self.get_font(size, bold);
        if font.is_null() {
            return None;
        }
        let c_text = CString::new(text).ok()?;
        let white = SDL_Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        let info = unsafe {
            let surf = TTF_RenderUTF8_Blended(font, c_text.as_ptr(), white);
            Self::surface_to_texture(surf, true)?
        };
        self.text_cache.insert(key, info);
        Some(info)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_rounded_rect(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        fill_top: [f32; 4],
        fill_bot: [f32; 4],
        border_width: f32,
        border_color: [f32; 4],
        glow_radius: f32,
        glow_color: [f32; 4],
        dashed: bool,
    ) {
        let pad = glow_radius.max(2.0);
        let qx = x - pad;
        let qy = y - pad;
        let qw = w + pad * 2.0;
        let qh = h + pad * 2.0;

        let unit_quad: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];

        unsafe {
            glUseProgram(self.rect_prog);
            glUniform2f(self.rect_u_res, self.width, self.height);
            glUniform4f(self.rect_u_quad, qx, qy, qw, qh);
            glUniform4f(self.rect_u_rect, x, y, w, h);
            glUniform1f(self.rect_u_radius, radius);
            glUniform4f(
                self.rect_u_fill_top,
                fill_top[0],
                fill_top[1],
                fill_top[2],
                fill_top[3],
            );
            glUniform4f(
                self.rect_u_fill_bot,
                fill_bot[0],
                fill_bot[1],
                fill_bot[2],
                fill_bot[3],
            );
            glUniform1f(self.rect_u_border_width, border_width);
            glUniform4f(
                self.rect_u_border_color,
                border_color[0],
                border_color[1],
                border_color[2],
                border_color[3],
            );
            glUniform1f(self.rect_u_glow_radius, glow_radius);
            glUniform4f(
                self.rect_u_glow_color,
                glow_color[0],
                glow_color[1],
                glow_color[2],
                glow_color[3],
            );
            glUniform1f(self.rect_u_dashed, if dashed { 1.0 } else { 0.0 });

            let attr = self.rect_a_unit as GLuint;
            glEnableVertexAttribArray(attr);
            glVertexAttribPointer(
                attr,
                2,
                GL_FLOAT,
                GL_FALSE,
                0,
                unit_quad.as_ptr() as *const c_void,
            );
            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
            glDisableVertexAttribArray(attr);
        }
    }

    pub fn draw_texture(
        &self,
        tex: TextureInfo,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        color: [f32; 4],
    ) {
        let unit_quad: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        unsafe {
            glUseProgram(self.tex_prog);
            glUniform2f(self.tex_u_res, self.width, self.height);
            glUniform4f(self.tex_u_quad, x, y, w, h);
            glUniform4f(self.tex_u_rect, x, y, w, h);
            glUniform1f(self.tex_u_radius, radius);
            glUniform4f(self.tex_u_color, color[0], color[1], color[2], color[3]);
            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_2D, tex.id);
            glUniform1i(self.tex_u_tex, 0);

            let attr = self.tex_a_unit as GLuint;
            glEnableVertexAttribArray(attr);
            glVertexAttribPointer(
                attr,
                2,
                GL_FLOAT,
                GL_FALSE,
                0,
                unit_quad.as_ptr() as *const c_void,
            );
            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
            glDisableVertexAttribArray(attr);
        }
    }

    pub fn draw_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        size: u16,
        bold: bool,
        color: [f32; 4],
        align_x: f32, // 0.0 = left, 0.5 = center, 1.0 = right
        align_y: f32, // 0.0 = top, 0.5 = middle, 1.0 = bottom
    ) -> (f32, f32) {
        if let Some(tex) = self.get_text_texture(text, size, bold) {
            let w = tex.w as f32;
            let h = tex.h as f32;
            let dx = (x - w * align_x).round();
            let dy = (y - h * align_y).round();
            self.draw_texture(tex, dx, dy, w, h, 0.0, color);
            (w, h)
        } else {
            (0.0, 0.0)
        }
    }

    pub fn push_tri_vert(&mut self, x: f32, y: f32, col: [f32; 4]) {
        self.tri_verts
            .extend_from_slice(&[x, y, col[0], col[1], col[2], col[3]]);
    }

    pub fn push_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, col: [f32; 4]) {
        self.line_verts.extend_from_slice(&[
            x1, y1, col[0], col[1], col[2], col[3], x2, y2, col[0], col[1], col[2], col[3],
        ]);
    }

    pub fn push_thick_line(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        thickness: f32,
        col: [f32; 4],
    ) {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return;
        }
        let nx = -dy / len * (thickness * 0.5);
        let ny = dx / len * (thickness * 0.5);
        self.push_tri_vert(x1 + nx, y1 + ny, col);
        self.push_tri_vert(x1 - nx, y1 - ny, col);
        self.push_tri_vert(x2 + nx, y2 + ny, col);

        self.push_tri_vert(x1 - nx, y1 - ny, col);
        self.push_tri_vert(x2 - nx, y2 - ny, col);
        self.push_tri_vert(x2 + nx, y2 + ny, col);
    }

    pub fn push_diamond(&mut self, cx: f32, cy: f32, r: f32, col: [f32; 4]) {
        self.push_tri_vert(cx, cy - r, col);
        self.push_tri_vert(cx + r, cy, col);
        self.push_tri_vert(cx, cy + r, col);

        self.push_tri_vert(cx, cy - r, col);
        self.push_tri_vert(cx, cy + r, col);
        self.push_tri_vert(cx - r, cy, col);
    }

    pub fn push_octagon(&mut self, cx: f32, cy: f32, r: f32, col: [f32; 4]) {
        const COS: [f32; 8] = [1.0, 0.7071, 0.0, -0.7071, -1.0, -0.7071, 0.0, 0.7071];
        const SIN: [f32; 8] = [0.0, 0.7071, 1.0, 0.7071, 0.0, -0.7071, -1.0, -0.7071];
        for i in 0..8 {
            let next = (i + 1) & 7;
            self.push_tri_vert(cx, cy, col);
            self.push_tri_vert(cx + COS[i] * r, cy + SIN[i] * r, col);
            self.push_tri_vert(cx + COS[next] * r, cy + SIN[next] * r, col);
        }
    }

    pub fn flush_primitives(&mut self, line_width: f32) {
        if self.line_verts.is_empty() && self.tri_verts.is_empty() {
            return;
        }
        unsafe {
            glUseProgram(self.prim_prog);
            glUniform2f(self.prim_u_res, self.width, self.height);
            let a_pos = self.prim_a_pos as GLuint;
            let a_col = self.prim_a_col as GLuint;
            glEnableVertexAttribArray(a_pos);
            glEnableVertexAttribArray(a_col);
            let stride = (6 * std::mem::size_of::<f32>()) as GLsizei;

            if !self.line_verts.is_empty() {
                glLineWidth(line_width);
                let ptr = self.line_verts.as_ptr();
                glVertexAttribPointer(a_pos, 2, GL_FLOAT, GL_FALSE, stride, ptr as *const c_void);
                glVertexAttribPointer(
                    a_col,
                    4,
                    GL_FLOAT,
                    GL_FALSE,
                    stride,
                    ptr.add(2) as *const c_void,
                );
                let count = (self.line_verts.len() / 6) as GLsizei;
                glDrawArrays(GL_LINES, 0, count);
                self.line_verts.clear();
            }

            if !self.tri_verts.is_empty() {
                let ptr = self.tri_verts.as_ptr();
                glVertexAttribPointer(a_pos, 2, GL_FLOAT, GL_FALSE, stride, ptr as *const c_void);
                glVertexAttribPointer(
                    a_col,
                    4,
                    GL_FLOAT,
                    GL_FALSE,
                    stride,
                    ptr.add(2) as *const c_void,
                );
                let count = (self.tri_verts.len() / 6) as GLsizei;
                glDrawArrays(GL_TRIANGLES, 0, count);
                self.tri_verts.clear();
            }

            glDisableVertexAttribArray(a_pos);
            glDisableVertexAttribArray(a_col);
        }
    }
}
