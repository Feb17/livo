//! macOS：mpv 用 OpenGL 渲染到离屏纹理，再用两个着色器 pass 转成 NV12 写进 IOSurface，
//! 交给 gpui::surface 直接显示，画面全程留在 GPU。思路同原项目 media_kit 的 TextureHW；
//! 转 NV12 是因为 GPUI 的 surface 只接受 420f（metal_renderer.rs 里有断言）。
use crate::mpv::{check, cstr, mpv_render_context_create, mpv_render_context_free, mpv_render_context_render, target_size, Backend, Frame, Handle, RenderParam, PARAM_API_TYPE};
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_video::pixel_buffer::{kCVPixelFormatType_420YpCbCr8BiPlanarFullRange, CVPixelBuffer, CVPixelBufferKeys};
use core_video::pixel_buffer_pool::CVPixelBufferPool;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void};

/// 一帧 NV12 画面；发送前已 glFinish，CF 对象的引用计数本身线程安全。
pub struct Nv12(pub CVPixelBuffer);
unsafe impl Send for Nv12 {}

type Cgl = *mut c_void;

#[link(name = "OpenGL", kind = "framework")]
extern "C" {
    fn CGLChoosePixelFormat(attrs: *const c_int, pix: *mut *mut c_void, npix: *mut c_int) -> c_int;
    fn CGLDestroyPixelFormat(pix: *mut c_void) -> c_int;
    fn CGLCreateContext(pix: *mut c_void, share: Cgl, ctx: *mut Cgl) -> c_int;
    fn CGLSetCurrentContext(ctx: Cgl) -> c_int;
    fn CGLDestroyContext(ctx: Cgl) -> c_int;
    fn CGLTexImageIOSurface2D(ctx: Cgl, target: u32, internal: u32, w: c_int, h: c_int, format: u32, ty: u32, surface: *mut c_void, plane: u32) -> c_int;
    fn glGenTextures(n: c_int, out: *mut u32);
    fn glDeleteTextures(n: c_int, ids: *const u32);
    fn glBindTexture(target: u32, id: u32);
    fn glTexImage2D(target: u32, level: c_int, internal: c_int, w: c_int, h: c_int, border: c_int, format: u32, ty: u32, data: *const c_void);
    fn glTexParameteri(target: u32, name: u32, value: c_int);
    fn glGenFramebuffers(n: c_int, out: *mut u32);
    fn glDeleteFramebuffers(n: c_int, ids: *const u32);
    fn glBindFramebuffer(target: u32, id: u32);
    fn glFramebufferTexture2D(target: u32, attachment: u32, tex_target: u32, tex: u32, level: c_int);
    fn glCheckFramebufferStatus(target: u32) -> u32;
    fn glViewport(x: c_int, y: c_int, w: c_int, h: c_int);
    fn glCreateShader(kind: u32) -> u32;
    fn glShaderSource(shader: u32, n: c_int, src: *const *const c_char, len: *const c_int);
    fn glCompileShader(shader: u32);
    fn glGetShaderiv(shader: u32, name: u32, out: *mut c_int);
    fn glGetShaderInfoLog(shader: u32, max: c_int, len: *mut c_int, log: *mut c_char);
    fn glCreateProgram() -> u32;
    fn glAttachShader(program: u32, shader: u32);
    fn glLinkProgram(program: u32);
    fn glGetProgramiv(program: u32, name: u32, out: *mut c_int);
    fn glUseProgram(program: u32);
    fn glGenVertexArrays(n: c_int, out: *mut u32);
    fn glBindVertexArray(id: u32);
    fn glDrawArrays(mode: u32, first: c_int, count: c_int);
    fn glActiveTexture(unit: u32);
    fn glFinish();
}

#[link(name = "IOSurface", kind = "framework")]
extern "C" {
    fn IOSurfaceGetID(surface: *mut c_void) -> u32;
}

extern "C" {
    fn CVPixelBufferGetIOSurface(pb: *mut c_void) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
}

// CGLTypes.h / gl3.h / render.h 中的常量
const CGL_PFA_ACCELERATED: c_int = 73;
const CGL_PFA_ALLOW_OFFLINE: c_int = 96;
const CGL_PFA_PROFILE: c_int = 99;
const CGL_PROFILE_3_2_CORE: c_int = 0x3200;
const GL_TRIANGLES: u32 = 0x0004;
const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_TEXTURE_RECTANGLE: u32 = 0x84F5;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_RED: u32 = 0x1903;
const GL_RG: u32 = 0x8227;
const GL_RGBA: u32 = 0x1908;
const GL_R8: u32 = 0x8229;
const GL_RG8: u32 = 0x822B;
const GL_RGBA8: u32 = 0x8058;
const GL_LINEAR: c_int = 0x2601;
const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
const GL_TEXTURE_WRAP_S: u32 = 0x2802;
const GL_TEXTURE_WRAP_T: u32 = 0x2803;
const GL_CLAMP_TO_EDGE: c_int = 0x812F;
const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
const GL_VERTEX_SHADER: u32 = 0x8B31;
const GL_FRAGMENT_SHADER: u32 = 0x8B30;
const GL_COMPILE_STATUS: u32 = 0x8B81;
const GL_LINK_STATUS: u32 = 0x8B82;
const GL_TEXTURE0: u32 = 0x84C0;
const PARAM_OPENGL_INIT: c_int = 2;
const PARAM_OPENGL_FBO: c_int = 3;
/// 超过 4K 的源按比例缩小，避免 IOSurface 过大。
const MAX_WIDTH: i64 = 3840;

#[repr(C)]
struct InitParams {
    get_proc_address: extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    ctx: *mut c_void,
}

#[repr(C)]
struct MpvFbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

extern "C" fn proc_address(_: *mut c_void, name: *const c_char) -> *mut c_void {
    // RTLD_DEFAULT：OpenGL.framework 已链接，直接按名字查
    unsafe { dlsym(-2isize as *mut c_void, name) }
}

// 全屏三角形，uv 覆盖 [0,1]
const VERT: &str = "#version 150
out vec2 uv;
void main() {
    vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2);
    uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}";

// BT.601 全范围，与 GPUI surface_fragment 的 ycbcrToRGBTransform 互逆
const FRAG_Y: &str = "#version 150
in vec2 uv; uniform sampler2D tex; out vec4 o;
void main() { o = vec4(dot(texture(tex, uv).rgb, vec3(0.299, 0.587, 0.114)), 0.0, 0.0, 1.0); }";

// 半分辨率视口下 uv 正好落在 2x2 块中心，线性采样即取平均
const FRAG_UV: &str = "#version 150
in vec2 uv; uniform sampler2D tex; out vec4 o;
void main() {
    vec3 c = texture(tex, uv).rgb;
    o = vec4(dot(c, vec3(-0.168736, -0.331264, 0.5)) + 0.5, dot(c, vec3(0.5, -0.418688, -0.081312)) + 0.5, 0.0, 1.0);
}";

pub struct Gl {
    cgl: Cgl,
    ctx: *mut c_void,
    prog_y: u32,
    prog_uv: u32,
    vao: u32,
    /// mpv 画面的中间纹理：(纹理, FBO, 宽, 高)
    src: (u32, u32, c_int, c_int),
    pool: Option<(c_int, c_int, CVPixelBufferPool)>,
    /// IOSurface ID -> [Y 纹理, Y FBO, UV 纹理, UV FBO]，池子循环复用同几块 surface
    planes: HashMap<u32, [u32; 4]>,
}

unsafe fn shader(kind: u32, src: &str) -> anyhow::Result<u32> {
    let s = glCreateShader(kind);
    let src = cstr(src);
    glShaderSource(s, 1, &src.as_ptr(), std::ptr::null());
    glCompileShader(s);
    let mut ok = 0;
    glGetShaderiv(s, GL_COMPILE_STATUS, &mut ok);
    if ok == 0 {
        let mut log = [0 as c_char; 512];
        glGetShaderInfoLog(s, log.len() as c_int, std::ptr::null_mut(), log.as_mut_ptr());
        anyhow::bail!("着色器编译失败：{}", std::ffi::CStr::from_ptr(log.as_ptr()).to_string_lossy());
    }
    Ok(s)
}

unsafe fn program(frag: &str) -> anyhow::Result<u32> {
    let p = glCreateProgram();
    glAttachShader(p, shader(GL_VERTEX_SHADER, VERT)?);
    glAttachShader(p, shader(GL_FRAGMENT_SHADER, frag)?);
    glLinkProgram(p);
    let mut ok = 0;
    glGetProgramiv(p, GL_LINK_STATUS, &mut ok);
    anyhow::ensure!(ok != 0, "着色器链接失败");
    Ok(p)
}

/// 把纹理挂到新 FBO 上，返回 FBO。
unsafe fn fbo_for(target: u32, tex: u32) -> anyhow::Result<u32> {
    let mut fbo = 0;
    glGenFramebuffers(1, &mut fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, fbo);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, target, tex, 0);
    let ok = glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE;
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    anyhow::ensure!(ok, "FBO 不完整");
    Ok(fbo)
}

impl Gl {
    pub fn new(mpv: *mut c_void) -> anyhow::Result<Self> {
        unsafe {
            let attrs = [CGL_PFA_PROFILE, CGL_PROFILE_3_2_CORE, CGL_PFA_ACCELERATED, CGL_PFA_ALLOW_OFFLINE, 0];
            let (mut pix, mut n) = (std::ptr::null_mut(), 0);
            anyhow::ensure!(CGLChoosePixelFormat(attrs.as_ptr(), &mut pix, &mut n) == 0 && !pix.is_null(), "没有可用的 OpenGL 3.2 像素格式");
            let mut cgl = std::ptr::null_mut();
            let err = CGLCreateContext(pix, std::ptr::null_mut(), &mut cgl);
            CGLDestroyPixelFormat(pix);
            anyhow::ensure!(err == 0, "CGL 上下文创建失败（{err}）");
            CGLSetCurrentContext(cgl);
            // 先构造出来，后面任何一步失败都由 Drop 收拾
            let mut gl = Gl { cgl, ctx: std::ptr::null_mut(), prog_y: 0, prog_uv: 0, vao: 0, src: (0, 0, 0, 0), pool: None, planes: HashMap::new() };
            gl.prog_y = program(FRAG_Y)?;
            gl.prog_uv = program(FRAG_UV)?;
            glGenVertexArrays(1, &mut gl.vao);
            let api = cstr("opengl");
            let mut init = InitParams { get_proc_address: proc_address, ctx: std::ptr::null_mut() };
            let mut params = [
                RenderParam { kind: PARAM_API_TYPE, data: api.as_ptr() as *mut c_void },
                RenderParam { kind: PARAM_OPENGL_INIT, data: &mut init as *mut InitParams as *mut c_void },
                RenderParam { kind: 0, data: std::ptr::null_mut() },
            ];
            check(mpv_render_context_create(&mut gl.ctx, mpv, params.as_mut_ptr()), "创建 OpenGL 渲染上下文")?;
            Ok(gl)
        }
    }

    unsafe fn ensure_src(&mut self, w: c_int, h: c_int) -> anyhow::Result<()> {
        if (self.src.2, self.src.3) == (w, h) {
            return Ok(());
        }
        glDeleteFramebuffers(1, &self.src.1);
        glDeleteTextures(1, &self.src.0);
        let mut tex = 0;
        glGenTextures(1, &mut tex);
        glBindTexture(GL_TEXTURE_2D, tex);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8 as c_int, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, std::ptr::null());
        for (k, v) in [(GL_TEXTURE_MIN_FILTER, GL_LINEAR), (GL_TEXTURE_MAG_FILTER, GL_LINEAR), (GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE), (GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE)] {
            glTexParameteri(GL_TEXTURE_2D, k, v);
        }
        self.src = (tex, 0, 0, 0);
        self.src = (tex, fbo_for(GL_TEXTURE_2D, tex)?, w, h);
        Ok(())
    }

    fn clear_planes(&mut self) {
        for [ty, fy, tuv, fuv] in self.planes.drain().map(|(_, v)| v) {
            unsafe {
                glDeleteFramebuffers(2, [fy, fuv].as_ptr());
                glDeleteTextures(2, [ty, tuv].as_ptr());
            }
        }
    }

    fn buffer(&mut self, w: c_int, h: c_int) -> anyhow::Result<CVPixelBuffer> {
        if !matches!(&self.pool, Some((pw, ph, _)) if (*pw, *ph) == (w, h)) {
            self.clear_planes();
            let key = |k: CVPixelBufferKeys| CFString::from(k);
            let empty: CFDictionary<CFString, CFNumber> = CFDictionary::from_CFType_pairs(&[]);
            let attrs = CFDictionary::from_CFType_pairs(&[
                (key(CVPixelBufferKeys::PixelFormatType), CFNumber::from(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange as i64).as_CFType()),
                (key(CVPixelBufferKeys::Width), CFNumber::from(w as i64).as_CFType()),
                (key(CVPixelBufferKeys::Height), CFNumber::from(h as i64).as_CFType()),
                (key(CVPixelBufferKeys::IOSurfaceProperties), empty.as_CFType()),
                (key(CVPixelBufferKeys::MetalCompatibility), CFBoolean::true_value().as_CFType()),
                (key(CVPixelBufferKeys::OpenGLCompatibility), CFBoolean::true_value().as_CFType()),
            ]);
            let pool = CVPixelBufferPool::new(None, Some(&attrs)).map_err(|e| anyhow::anyhow!("CVPixelBufferPool 创建失败（{e}）"))?;
            self.pool = Some((w, h, pool));
        }
        let (_, _, pool) = self.pool.as_ref().unwrap();
        pool.create_pixel_buffer().map_err(|e| anyhow::anyhow!("取 CVPixelBuffer 失败（{e}）"))
    }

    /// 给这块 IOSurface 的 Y/UV 两个平面各绑一个可渲染的纹理+FBO（按 surface ID 缓存）。
    unsafe fn planes_for(&mut self, pb: &CVPixelBuffer, w: c_int, h: c_int) -> anyhow::Result<[u32; 4]> {
        let surface = CVPixelBufferGetIOSurface(pb.as_concrete_TypeRef() as *mut c_void);
        anyhow::ensure!(!surface.is_null(), "CVPixelBuffer 没有 IOSurface");
        let id = IOSurfaceGetID(surface);
        if let Some(p) = self.planes.get(&id) {
            return Ok(*p);
        }
        // 池子偶尔会换新 surface，旧的缓存无从得知何时失效，攒多了就整体清掉
        if self.planes.len() >= 8 {
            self.clear_planes();
        }
        let mut out = [0u32; 4];
        for (i, (plane, internal, format, pw, ph)) in [(0, GL_R8, GL_RED, w, h), (1, GL_RG8, GL_RG, w / 2, h / 2)].into_iter().enumerate() {
            let mut tex = 0;
            glGenTextures(1, &mut tex);
            glBindTexture(GL_TEXTURE_RECTANGLE, tex);
            let err = CGLTexImageIOSurface2D(self.cgl, GL_TEXTURE_RECTANGLE, internal, pw, ph, format, GL_UNSIGNED_BYTE, surface, plane);
            anyhow::ensure!(err == 0, "IOSurface 平面 {plane} 绑定失败（{err}）");
            out[i * 2] = tex;
            out[i * 2 + 1] = fbo_for(GL_TEXTURE_RECTANGLE, tex)?;
        }
        self.planes.insert(id, out);
        Ok(out)
    }

    unsafe fn draw(&mut self, w: c_int, h: c_int) -> anyhow::Result<Option<CVPixelBuffer>> {
        self.ensure_src(w, h)?;
        let mut fbo = MpvFbo { fbo: self.src.1 as c_int, w, h, internal_format: 0 };
        // 不翻转：mpv 默认输出里 GL 第 0 行就是画面顶部，正好对应 IOSurface 内存第 0 行（测试校验）
        let mut params = [
            RenderParam { kind: PARAM_OPENGL_FBO, data: &mut fbo as *mut MpvFbo as *mut c_void },
            RenderParam { kind: 0, data: std::ptr::null_mut() },
        ];
        if mpv_render_context_render(self.ctx, params.as_mut_ptr()) < 0 {
            return Ok(None);
        }
        let pb = self.buffer(w, h)?;
        let [_, fy, _, fuv] = self.planes_for(&pb, w, h)?;
        glBindVertexArray(self.vao);
        glActiveTexture(GL_TEXTURE0);
        glBindTexture(GL_TEXTURE_2D, self.src.0);
        for (fb, prog, vw, vh) in [(fy, self.prog_y, w, h), (fuv, self.prog_uv, w / 2, h / 2)] {
            glBindFramebuffer(GL_FRAMEBUFFER, fb);
            glViewport(0, 0, vw, vh);
            glUseProgram(prog);
            glDrawArrays(GL_TRIANGLES, 0, 3);
        }
        glBindFramebuffer(GL_FRAMEBUFFER, 0);
        // GPUI 用 Metal 读同一块 IOSurface，必须等 GL 写完
        glFinish();
        Ok(Some(pb))
    }
}

impl Backend for Gl {
    fn ctx(&self) -> *mut c_void {
        self.ctx
    }

    fn hwdec(&self) -> &'static str {
        "auto-safe"
    }

    fn render(&mut self, mpv: &Handle, view: u64) -> Option<Frame> {
        let (w, h) = target_size(mpv, view, MAX_WIDTH)?;
        // NV12 色度是半分辨率，宽高取偶数
        let (w, h) = (w & !1, h & !1);
        match unsafe { self.draw(w as c_int, h as c_int) } {
            Ok(pb) => pb.map(|pb| Frame::Surface(Nv12(pb))),
            Err(e) => {
                eprintln!("OpenGL 渲染失败：{e}");
                None
            }
        }
    }
}

impl Drop for Gl {
    fn drop(&mut self) {
        unsafe {
            CGLSetCurrentContext(self.cgl);
            if !self.ctx.is_null() {
                mpv_render_context_free(self.ctx);
            }
            // 纹理/FBO/着色器随上下文一起销毁
            self.planes.clear();
            self.pool = None;
            CGLSetCurrentContext(std::ptr::null_mut());
            CGLDestroyContext(self.cgl);
        }
    }
}

#[cfg(test)]
impl Nv12 {
    /// 测试用：读回 NV12，检查尺寸、方向（左上角白块）和色度中性。
    pub fn assert_test_source(&self, w: usize, h: usize) {
        use core_video::pixel_buffer::{CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferLockBaseAddress, CVPixelBufferUnlockBaseAddress};
        let pb = self.0.as_concrete_TypeRef();
        assert_eq!((self.0.get_width(), self.0.get_height()), (w, h));
        assert_eq!(self.0.get_pixel_format(), kCVPixelFormatType_420YpCbCr8BiPlanarFullRange);
        unsafe {
            CVPixelBufferLockBaseAddress(pb, 1);
            let (y, ys) = (CVPixelBufferGetBaseAddressOfPlane(pb, 0) as *const u8, CVPixelBufferGetBytesPerRowOfPlane(pb, 0));
            let (uv, uvs) = (CVPixelBufferGetBaseAddressOfPlane(pb, 1) as *const u8, CVPixelBufferGetBytesPerRowOfPlane(pb, 1));
            let luma = |x: usize, row: usize| *y.add(row * ys + x);
            let (top_left, bottom_left, top_right) = (luma(10, 10), luma(10, h - 10), luma(w - 10, 10));
            let (cb, cr) = (*uv.add(5 * uvs + 10), *uv.add(5 * uvs + 11));
            CVPixelBufferUnlockBaseAddress(pb, 1);
            assert!(top_left > 230 && bottom_left < 25 && top_right < 25, "方向或亮度不对：{top_left} {bottom_left} {top_right}");
            assert!(cb.abs_diff(128) < 8 && cr.abs_diff(128) < 8, "白色的色度应接近 128：{cb} {cr}");
        }
    }
}
