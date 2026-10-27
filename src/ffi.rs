#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]

use libc::{c_char, c_float, c_int, c_uint, c_ushort, c_void};
use std::ffi::CString;

pub type SDL_Window = c_void;
pub type SDL_GLContext = *mut c_void;
pub type TTF_Font = c_void;

pub const SDL_INIT_TIMER: u32 = 0x0000_0001;
pub const SDL_INIT_VIDEO: u32 = 0x0000_0020;
pub const SDL_INIT_EVENTS: u32 = 0x0000_4000;

pub const SDL_WINDOW_OPENGL: u32 = 0x0000_0002;
pub const SDL_WINDOW_SHOWN: u32 = 0x0000_0004;
pub const SDL_WINDOW_BORDERLESS: u32 = 0x0000_0010;
pub const SDL_WINDOW_FULLSCREEN: u32 = 0x0000_0001;

pub const SDL_WINDOWPOS_UNDEFINED: c_int = 0x1FFF_0000;

pub const SDL_GL_RED_SIZE: c_int = 0;
pub const SDL_GL_GREEN_SIZE: c_int = 1;
pub const SDL_GL_BLUE_SIZE: c_int = 2;
pub const SDL_GL_ALPHA_SIZE: c_int = 3;
pub const SDL_GL_DOUBLEBUFFER: c_int = 5;
pub const SDL_GL_DEPTH_SIZE: c_int = 6;
pub const SDL_GL_MULTISAMPLEBUFFERS: c_int = 13;
pub const SDL_GL_MULTISAMPLESAMPLES: c_int = 14;
pub const SDL_GL_CONTEXT_MAJOR_VERSION: c_int = 17;
pub const SDL_GL_CONTEXT_MINOR_VERSION: c_int = 18;
pub const SDL_GL_CONTEXT_PROFILE_MASK: c_int = 21;
pub const SDL_GL_CONTEXT_PROFILE_ES: c_int = 0x0004;

// Little-endian RGBA32 (byte order R, G, B, A in memory)
pub const SDL_PIXELFORMAT_ABGR8888: u32 = 0x1676_2004;

pub const IMG_INIT_PNG: c_int = 0x0000_0002;

pub const SDL_QUIT: u32 = 0x100;
pub const SDL_WINDOWEVENT: u32 = 0x200;
pub const SDL_KEYDOWN: u32 = 0x300;
pub const SDL_KEYUP: u32 = 0x301;
pub const SDL_MOUSEMOTION: u32 = 0x400;
pub const SDL_MOUSEBUTTONDOWN: u32 = 0x401;
pub const SDL_MOUSEBUTTONUP: u32 = 0x402;
pub const SDL_MOUSEWHEEL: u32 = 0x403;

pub const SDL_WINDOWEVENT_SHOWN: u8 = 1;
pub const SDL_WINDOWEVENT_HIDDEN: u8 = 2;
pub const SDL_WINDOWEVENT_EXPOSED: u8 = 3;
pub const SDL_WINDOWEVENT_MINIMIZED: u8 = 7;
pub const SDL_WINDOWEVENT_RESTORED: u8 = 9;
pub const SDL_WINDOWEVENT_FOCUS_GAINED: u8 = 12;
pub const SDL_WINDOWEVENT_FOCUS_LOST: u8 = 13;

// Scancodes (USB HID + webOS extensions)
pub const SDL_SCANCODE_RETURN: i32 = 40;
pub const SDL_SCANCODE_ESCAPE: i32 = 41;
pub const SDL_SCANCODE_BACKSPACE: i32 = 42;
pub const SDL_SCANCODE_SPACE: i32 = 44;
pub const SDL_SCANCODE_RIGHT: i32 = 79;
pub const SDL_SCANCODE_LEFT: i32 = 80;
pub const SDL_SCANCODE_DOWN: i32 = 81;
pub const SDL_SCANCODE_UP: i32 = 82;
pub const SDL_SCANCODE_KP_ENTER: i32 = 88;
pub const SDL_SCANCODE_AC_BACK: i32 = 270;
pub const SDL_SCANCODE_AC_HOME: i32 = 269;
pub const SDL_WEBOS_SCANCODE_BACK: i32 = 482;
pub const SDL_WEBOS_SCANCODE_EXIT: i32 = 483;
pub const SDL_WEBOS_SCANCODE_RED: i32 = 484;
pub const SDL_WEBOS_SCANCODE_GREEN: i32 = 485;
pub const SDL_WEBOS_SCANCODE_YELLOW: i32 = 486;
pub const SDL_WEBOS_SCANCODE_BLUE: i32 = 487;

// Keycodes (SDL_Keycode)
pub const SDLK_RETURN: i32 = 13;
pub const SDLK_ESCAPE: i32 = 27;
pub const SDLK_BACKSPACE: i32 = 8;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
pub struct SDL_Surface {
    pub flags: u32,
    pub format: *mut c_void,
    pub w: c_int,
    pub h: c_int,
    pub pitch: c_int,
    pub pixels: *mut c_void,
    pub userdata: *mut c_void,
    pub locked: c_int,
    pub lock_data: *mut c_void,
    pub clip_rect: [c_int; 4],
    pub map: *mut c_void,
    pub refcount: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_Keysym {
    pub scancode: i32,
    pub sym: i32,
    pub mod_: c_ushort,
    pub unused: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_KeyboardEvent {
    pub type_: u32,
    pub timestamp: u32,
    pub window_id: u32,
    pub which: u32, // LG webOS SDL2 custom MRCU device ID field at offset 0x0c
    pub state: u8,
    pub repeat: u8,
    pub padding2: u8,
    pub padding3: u8,
    pub keysym: SDL_Keysym,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_WindowEvent {
    pub type_: u32,
    pub timestamp: u32,
    pub window_id: u32,
    pub event: u8,
    pub padding1: u8,
    pub padding2: u8,
    pub padding3: u8,
    pub data1: i32,
    pub data2: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_MouseMotionEvent {
    pub type_: u32,
    pub timestamp: u32,
    pub window_id: u32,
    pub which: u32,
    pub state: u32,
    pub x: i32,
    pub y: i32,
    pub xrel: i32,
    pub yrel: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_MouseButtonEvent {
    pub type_: u32,
    pub timestamp: u32,
    pub window_id: u32,
    pub which: u32,
    pub button: u8,
    pub state: u8,
    pub clicks: u8,
    pub padding1: u8,
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SDL_MouseWheelEvent {
    pub type_: u32,
    pub timestamp: u32,
    pub window_id: u32,
    pub which: u32,
    pub x: i32,
    pub y: i32,
    pub direction: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union SDL_Event {
    pub type_: u32,
    pub window: SDL_WindowEvent,
    pub key: SDL_KeyboardEvent,
    pub motion: SDL_MouseMotionEvent,
    pub button: SDL_MouseButtonEvent,
    pub wheel: SDL_MouseWheelEvent,
    pub padding: [u8; 64],
}

// OpenGL ES 2.0 constants & types
pub type GLenum = c_uint;
pub type GLboolean = u8;
pub type GLbitfield = c_uint;
pub type GLint = c_int;
pub type GLsizei = c_int;
pub type GLuint = c_uint;
pub type GLfloat = c_float;

pub const GL_FALSE: GLboolean = 0;
pub const GL_TRUE: GLboolean = 1;
pub const GL_COLOR_BUFFER_BIT: GLbitfield = 0x0000_4000;
pub const GL_TRIANGLES: GLenum = 0x0004;
pub const GL_TRIANGLE_STRIP: GLenum = 0x0005;
pub const GL_LINES: GLenum = 0x0001;
pub const GL_BLEND: GLenum = 0x0BE2;
pub const GL_SCISSOR_TEST: GLenum = 0x0C11;
pub const GL_SRC_ALPHA: GLenum = 0x0302;
pub const GL_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
pub const GL_ONE: GLenum = 0x0001;
pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
pub const GL_TEXTURE0: GLenum = 0x84C0;
pub const GL_RGBA: GLenum = 0x1908;
pub const GL_UNSIGNED_BYTE: GLenum = 0x1401;
pub const GL_FLOAT: GLenum = 0x1406;
pub const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
pub const GL_TEXTURE_WRAP_T: GLenum = 0x2803;
pub const GL_NEAREST: GLint = 0x2600;
pub const GL_LINEAR: GLint = 0x2601;
pub const GL_CLAMP_TO_EDGE: GLint = 0x812F;

pub const TTF_HINTING_NORMAL: c_int = 0;
pub const TTF_HINTING_LIGHT: c_int = 1;
pub const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
pub const GL_VERTEX_SHADER: GLenum = 0x8B31;
pub const GL_COMPILE_STATUS: GLenum = 0x8B81;
pub const GL_LINK_STATUS: GLenum = 0x8B82;
pub const GL_UNPACK_ALIGNMENT: GLenum = 0x0CF5;

extern "C" {
    // SDL2 core
    pub fn SDL_Init(flags: u32) -> c_int;
    pub fn SDL_Quit();
    pub fn SDL_SetHint(name: *const c_char, value: *const c_char) -> c_int;
    pub fn SDL_GetError() -> *const c_char;
    pub fn SDL_GetTicks() -> u32;
    pub fn SDL_Delay(ms: u32);
    pub fn SDL_ShowCursor(toggle: c_int) -> c_int;

    pub fn SDL_GL_SetAttribute(attr: c_int, value: c_int) -> c_int;
    pub fn SDL_CreateWindow(
        title: *const c_char,
        x: c_int,
        y: c_int,
        w: c_int,
        h: c_int,
        flags: u32,
    ) -> *mut SDL_Window;
    pub fn SDL_DestroyWindow(window: *mut SDL_Window);
    pub fn SDL_GL_CreateContext(window: *mut SDL_Window) -> SDL_GLContext;
    pub fn SDL_GL_DeleteContext(context: SDL_GLContext);
    pub fn SDL_GL_SetSwapInterval(interval: c_int) -> c_int;
    pub fn SDL_GL_SwapWindow(window: *mut SDL_Window);
    pub fn SDL_GL_GetDrawableSize(window: *mut SDL_Window, w: *mut c_int, h: *mut c_int);
    pub fn SDL_GL_GetProcAddress(proc_: *const c_char) -> *const c_void;

    pub fn SDL_PollEvent(event: *mut SDL_Event) -> c_int;
    pub fn SDL_WaitEventTimeout(event: *mut SDL_Event, timeout: c_int) -> c_int;

    pub fn SDL_FreeSurface(surface: *mut SDL_Surface);
    pub fn SDL_ConvertSurfaceFormat(
        src: *mut SDL_Surface,
        pixel_format: u32,
        flags: u32,
    ) -> *mut SDL_Surface;

    // SDL2_image
    pub fn IMG_Init(flags: c_int) -> c_int;
    pub fn IMG_Quit();
    pub fn IMG_Load(file: *const c_char) -> *mut SDL_Surface;

    // SDL2_ttf
    pub fn TTF_Init() -> c_int;
    pub fn TTF_Quit();
    pub fn TTF_OpenFont(file: *const c_char, ptsize: c_int) -> *mut TTF_Font;
    pub fn TTF_CloseFont(font: *mut TTF_Font);
    pub fn TTF_SetFontHinting(font: *mut TTF_Font, hinting: c_int);
    pub fn TTF_RenderUTF8_Blended(
        font: *mut TTF_Font,
        text: *const c_char,
        fg: SDL_Color,
    ) -> *mut SDL_Surface;
}

// Dynamically loaded GLES 2.0 function pointers via SDL_GL_GetProcAddress
macro_rules! gl_procs {
    ($($ptr_name:ident => pub fn $name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(
            pub static mut $ptr_name: Option<unsafe extern "C" fn($($ty),*) -> $ret> = None;
            #[inline(always)]
            pub unsafe fn $name($($arg: $ty),*) -> $ret {
                if let Some(f) = $ptr_name {
                    f($($arg),*)
                } else {
                    std::mem::zeroed()
                }
            }
        )*

        pub unsafe fn gl_load_functions() {
            $(
                let cname = CString::new(stringify!($name)).unwrap();
                let ptr = SDL_GL_GetProcAddress(cname.as_ptr());
                if !ptr.is_null() {
                    $ptr_name = Some(std::mem::transmute(ptr));
                }
            )*
        }
    };
}

gl_procs! {
    PTR_glViewport => pub fn glViewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei) -> ();
    PTR_glClearColor => pub fn glClearColor(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) -> ();
    PTR_glClear => pub fn glClear(mask: GLbitfield) -> ();
    PTR_glEnable => pub fn glEnable(cap: GLenum) -> ();
    PTR_glDisable => pub fn glDisable(cap: GLenum) -> ();
    PTR_glBlendFunc => pub fn glBlendFunc(sfactor: GLenum, dfactor: GLenum) -> ();
    PTR_glScissor => pub fn glScissor(x: GLint, y: GLint, width: GLsizei, height: GLsizei) -> ();
    PTR_glPixelStorei => pub fn glPixelStorei(pname: GLenum, param: GLint) -> ();
    PTR_glLineWidth => pub fn glLineWidth(width: GLfloat) -> ();

    PTR_glCreateShader => pub fn glCreateShader(type_: GLenum) -> GLuint;
    PTR_glShaderSource => pub fn glShaderSource(shader: GLuint, count: GLsizei, string: *const *const c_char, length: *const GLint) -> ();
    PTR_glCompileShader => pub fn glCompileShader(shader: GLuint) -> ();
    PTR_glGetShaderiv => pub fn glGetShaderiv(shader: GLuint, pname: GLenum, params: *mut GLint) -> ();
    PTR_glDeleteShader => pub fn glDeleteShader(shader: GLuint) -> ();

    PTR_glCreateProgram => pub fn glCreateProgram() -> GLuint;
    PTR_glAttachShader => pub fn glAttachShader(program: GLuint, shader: GLuint) -> ();
    PTR_glLinkProgram => pub fn glLinkProgram(program: GLuint) -> ();
    PTR_glGetProgramiv => pub fn glGetProgramiv(program: GLuint, pname: GLenum, params: *mut GLint) -> ();
    PTR_glUseProgram => pub fn glUseProgram(program: GLuint) -> ();

    PTR_glGetAttribLocation => pub fn glGetAttribLocation(program: GLuint, name: *const c_char) -> GLint;
    PTR_glGetUniformLocation => pub fn glGetUniformLocation(program: GLuint, name: *const c_char) -> GLint;
    PTR_glEnableVertexAttribArray => pub fn glEnableVertexAttribArray(index: GLuint) -> ();
    PTR_glDisableVertexAttribArray => pub fn glDisableVertexAttribArray(index: GLuint) -> ();
    PTR_glVertexAttribPointer => pub fn glVertexAttribPointer(index: GLuint, size: GLint, type_: GLenum, normalized: GLboolean, stride: GLsizei, pointer: *const c_void) -> ();

    PTR_glUniform1i => pub fn glUniform1i(location: GLint, v0: GLint) -> ();
    PTR_glUniform1f => pub fn glUniform1f(location: GLint, v0: GLfloat) -> ();
    PTR_glUniform2f => pub fn glUniform2f(location: GLint, v0: GLfloat, v1: GLfloat) -> ();
    PTR_glUniform4f => pub fn glUniform4f(location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat, v3: GLfloat) -> ();

    PTR_glGenTextures => pub fn glGenTextures(n: GLsizei, textures: *mut GLuint) -> ();
    PTR_glDeleteTextures => pub fn glDeleteTextures(n: GLsizei, textures: *const GLuint) -> ();
    PTR_glActiveTexture => pub fn glActiveTexture(texture: GLenum) -> ();
    PTR_glBindTexture => pub fn glBindTexture(target: GLenum, texture: GLuint) -> ();
    PTR_glTexParameteri => pub fn glTexParameteri(target: GLenum, pname: GLenum, param: GLint) -> ();
    PTR_glTexImage2D => pub fn glTexImage2D(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, height: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) -> ();

    PTR_glDrawArrays => pub fn glDrawArrays(mode: GLenum, first: GLint, count: GLsizei) -> ();
}
