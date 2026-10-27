//! libmpv 播放：mpv 解码+出声，后台线程把画面交给 GPUI。
//! macOS 走 OpenGL → NV12 IOSurface（GPU 全程，见 mpv_gl.rs）；其他平台或 GL 不可用时走软件渲染。
use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

#[repr(C)]
pub(crate) struct RenderParam {
    pub kind: c_int,
    pub data: *mut c_void,
}

#[link(name = "mpv")]
extern "C" {
    fn mpv_create() -> *mut c_void;
    fn mpv_initialize(h: *mut c_void) -> c_int;
    fn mpv_set_option_string(h: *mut c_void, name: *const c_char, data: *const c_char) -> c_int;
    fn mpv_set_property_string(h: *mut c_void, name: *const c_char, data: *const c_char) -> c_int;
    fn mpv_command(h: *mut c_void, args: *mut *const c_char) -> c_int;
    fn mpv_get_property(h: *mut c_void, name: *const c_char, format: c_int, data: *mut c_void) -> c_int;
    fn mpv_get_property_string(h: *mut c_void, name: *const c_char) -> *mut c_char;
    fn mpv_free(data: *mut c_void);
    fn mpv_error_string(err: c_int) -> *const c_char;
    fn mpv_terminate_destroy(h: *mut c_void);
    pub(crate) fn mpv_render_context_create(res: *mut *mut c_void, h: *mut c_void, params: *mut RenderParam) -> c_int;
    fn mpv_render_context_set_update_callback(ctx: *mut c_void, cb: Option<extern "C" fn(*mut c_void)>, cb_ctx: *mut c_void);
    fn mpv_render_context_update(ctx: *mut c_void) -> u64;
    pub(crate) fn mpv_render_context_render(ctx: *mut c_void, params: *mut RenderParam) -> c_int;
    pub(crate) fn mpv_render_context_free(ctx: *mut c_void);
}

// render.h / client.h 中的常量
pub(crate) const PARAM_API_TYPE: c_int = 1;
const PARAM_SW_SIZE: c_int = 17;
const PARAM_SW_FORMAT: c_int = 18;
const PARAM_SW_STRIDE: c_int = 19;
const PARAM_SW_POINTER: c_int = 20;
const UPDATE_FRAME: u64 = 1;
const FORMAT_FLAG: c_int = 3;
const FORMAT_INT64: c_int = 4;
const FORMAT_DOUBLE: c_int = 5;
/// shortcut: 软件渲染整帧走 CPU，宽度封顶 1280 控制开销；只是 GL 不可用时的兜底。
const SW_MAX_WIDTH: i64 = 1280;

pub enum Frame {
    /// 软件渲染的 BGRA 像素（alpha 已置 255），交给 gpui::RenderImage。
    Bgra { width: u32, height: u32, bgra: Vec<u8> },
    /// GPU 渲染好的 NV12 画面，交给 gpui::surface。
    #[cfg(target_os = "macos")]
    Surface(crate::mpv_gl::Nv12),
}

/// 渲染后端：在渲染线程上创建和使用，Drop 时释放 mpv 渲染上下文。
pub(crate) trait Backend {
    fn ctx(&self) -> *mut c_void;
    /// 该后端配套的 hwdec：GL 可以直接用硬解纹理，软件渲染只能用 -copy。
    fn hwdec(&self) -> &'static str;
    /// `view` 是控件的物理像素尺寸（w<<32|h，0 表示还不知道）。
    fn render(&mut self, mpv: &Handle, view: u64) -> Option<Frame>;
}

pub(crate) struct Handle(pub *mut c_void);
// mpv_handle 的客户端 API 是线程安全的
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

pub struct Player {
    /// 视频控件的物理像素尺寸（w<<32|h），由界面每帧写入，渲染线程按它缩放输出。
    pub view: Arc<AtomicU64>,
    mpv: Arc<Handle>,
    stop: Arc<AtomicBool>,
    wake: mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

const MPV_ERROR_OPTION_NOT_FOUND: c_int = -5;

pub(crate) fn check(code: c_int, what: &str) -> anyhow::Result<()> {
    if code < 0 {
        let msg = unsafe { std::ffi::CStr::from_ptr(mpv_error_string(code)) }.to_string_lossy();
        anyhow::bail!("libmpv {what} 失败：{msg}");
    }
    Ok(())
}

pub(crate) fn cstr(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap()
}

extern "C" fn on_update(ctx: *mut c_void) {
    // 只能做轻量通知，不能回调 mpv
    let _ = unsafe { &*(ctx as *const mpsc::Sender<()>) }.send(());
}

fn make_backend(mpv: &Handle) -> anyhow::Result<Box<dyn Backend>> {
    #[cfg(target_os = "macos")]
    match crate::mpv_gl::Gl::new(mpv.0) {
        Ok(b) => return Ok(Box::new(b)),
        Err(e) => eprintln!("OpenGL 渲染不可用，改用软件渲染：{e}"),
    }
    Ok(Box::new(Sw::new(mpv.0)?))
}

impl Player {
    /// 按线路顺序播放 `urls`（打不开或断流时 mpv 自动切到下一条），
    /// 帧从返回的接收端送出（满了就丢帧，只保留最新画面）。
    /// `hw`：是否硬件解码；`buffer_mb`：网络缓冲上限，往回保留的缓存同样大，决定能回看多久（直播设置里可调）
    pub fn new(urls: &[String], headers: &[(String, String)], hw: bool, buffer_mb: u32) -> anyhow::Result<(Player, tokio::sync::mpsc::Receiver<Frame>)> {
        let h = unsafe { mpv_create() };
        anyhow::ensure!(!h.is_null(), "libmpv 创建失败");
        let header = |k: &str| headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map_or("", |(_, v)| v.as_str());
        // 时移：往回保留的数据和前向缓冲一样大，可回看的时长随缓冲区大小变化
        let (buf, back) = (format!("{}MiB", buffer_mb.max(1)), format!("{}MiB", buffer_mb.max(1)));
        let opts = [
            ("vo", "libmpv"),
            ("terminal", "no"),
            ("ytdl", "no"),
            ("referrer", header("referer")),
            ("user-agent", header("user-agent")),
            // 直播用的网络默认值，参照原项目 MpvOptionsService.networkDefaultOptions，但不开 reconnect_streamed：
            // 斗鱼/虎牙的地址几分钟就过期，断开后拿旧地址重连只会每秒重连几次、每次重新握手，画面也只剩碎片。
            // 断了就让这条线路结束，换下一条；都断了由界面的断流看门狗重新取地址
            ("network-timeout", "30"),
            ("stream-lavf-o", "reconnect=1,reconnect_on_network_error=1,reconnect_delay_max=10"),
            ("cache", "yes"),
            ("demuxer-readahead-secs", "2"),
            ("demuxer-max-bytes", &buf),
            ("demuxer-max-back-bytes", &back),
            // 只当播放内核用：libmpv 默认还会起 6 个 Lua 脚本线程（统计/控制台/选择器…）、加载字幕和字体，这里都用不上。
            // 实测去掉后 CPU 约降 1 个百分点、内存少 7MB 左右；精简版 libmpv 没编 Lua，这些选项不存在时下面会跳过
            ("load-scripts", "no"),
            ("load-stats-overlay", "no"),
            ("load-console", "no"),
            ("load-select", "no"),
            ("load-positioning", "no"),
            ("load-commands", "no"),
            ("load-context-menu", "no"),
            ("osc", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("sid", "no"),
            ("sub-auto", "no"),
            ("embeddedfonts", "no"),
            ("osd-level", "0"),
            ("audio-display", "no"),
            // 硬解时 4 个软解帧线程只会互相抢锁（实测每路约 2 个百分点 CPU）；软解才需要多线程，所以只在硬解时限制
            ("vd-lavc-threads", if hw { "2" } else { "" }),
            // 时移：直播流本身不能跳，只在已缓存的范围里跳（范围见 timeshift，界面会限制在这个范围内）
            ("force-seekable", "yes"),
            ("demuxer-seekable-cache", "yes"),
        ];
        let init = || -> anyhow::Result<()> {
            for (k, v) in opts.iter().filter(|(_, v)| !v.is_empty()) {
                let code = unsafe { mpv_set_option_string(h, cstr(k).as_ptr(), cstr(v).as_ptr()) };
                // 精简版 libmpv 没编 Lua，ytdl 之类的选项不存在，跳过即可
                if code != MPV_ERROR_OPTION_NOT_FOUND {
                    check(code, k)?;
                }
            }
            // mpv 默认的高质量缩放会按片源分辨率开 rgba16f 中间帧，4K 直播多占约 100MB 显存，直播画面用 fast 档足够。
            // 老版本 libmpv 没有这个档，失败就忽略
            unsafe { mpv_set_option_string(h, cstr("profile").as_ptr(), cstr("fast").as_ptr()) };
            check(unsafe { mpv_initialize(h) }, "初始化")
        };
        if let Err(e) = init() {
            unsafe { mpv_terminate_destroy(h) };
            return Err(e);
        }

        let (wake, wake_rx) = mpsc::channel();
        let (frame_tx, frame_rx) = tokio::sync::mpsc::channel(1);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let view = Arc::new(AtomicU64::new(0));
        let mpv = Arc::new(Handle(h));
        let cb = Handle(Box::into_raw(Box::new(wake.clone())) as *mut c_void);
        let (mpv2, stop2, view2) = (mpv.clone(), stop.clone(), view.clone());
        // GL 上下文必须在使用它的线程上创建，所以后端在渲染线程里建
        let thread = std::thread::spawn(move || {
            // 整体移入闭包（只捕获 .0 字段会丢掉 Send）
            let cb = cb;
            let mut backend = match make_backend(&mpv2) {
                Ok(b) => b,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    unsafe { drop(Box::from_raw(cb.0 as *mut mpsc::Sender<()>)) };
                    return;
                }
            };
            unsafe { mpv_render_context_set_update_callback(backend.ctx(), Some(on_update), cb.0) };
            let _ = ready_tx.send(Ok(backend.hwdec()));
            while wake_rx.recv().is_ok() && !stop2.load(Ordering::Relaxed) {
                if unsafe { mpv_render_context_update(backend.ctx()) } & UPDATE_FRAME == 0 {
                    continue;
                }
                let Some(frame) = backend.render(&mpv2, view2.load(Ordering::Relaxed)) else { continue };
                if let Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) = frame_tx.try_send(frame) {
                    break;
                }
            }
            // 后端释放渲染上下文后不会再有回调，此时才能释放回调里用的 Sender
            drop(backend);
            unsafe { drop(Box::from_raw(cb.0 as *mut mpsc::Sender<()>)) };
        });

        let player = Player { view, mpv, stop, wake, thread: Some(thread) };
        let hwdec = ready_rx.recv().map_err(|_| anyhow::anyhow!("渲染线程意外退出"))??;
        let hwdec = if hw { hwdec } else { "no" };
        check(unsafe { mpv_set_option_string(h, cstr("hwdec").as_ptr(), cstr(hwdec).as_ptr()) }, "hwdec")?;
        let (load, append) = (cstr("loadfile"), cstr("append-play"));
        for url in urls {
            let url = cstr(url);
            let mut cmd = [load.as_ptr(), url.as_ptr(), append.as_ptr(), std::ptr::null()];
            check(unsafe { mpv_command(player.mpv.0, cmd.as_mut_ptr()) }, "加载")?;
        }
        Ok((player, frame_rx))
    }
}

impl Player {
    /// 所有线路都播完/断开了
    pub fn idle(&self) -> bool {
        let mut v: c_int = 0;
        unsafe { mpv_get_property(self.mpv.0, cstr("idle-active").as_ptr(), FORMAT_FLAG, &mut v as *mut c_int as *mut c_void) };
        v != 0
    }

    /// 调试信息面板的内容：(名称, 值)；取不到的项（如直播流没有的码率）直接省略
    pub fn stats(&self) -> Vec<(&'static str, String)> {
        let h = &self.mpv;
        let (s, f, i) = (|n| prop_str(h, n), |n| prop_f64(h, n), |n| prop(h, n));
        let mbps = |bits: f64| format!("{:.2} Mbps", bits / 1e6);
        let join = |parts: Vec<Option<String>>| parts.into_iter().flatten().collect::<Vec<_>>().join(", ");
        let video = join(vec![
            s("video-format").map(|c| c.to_uppercase()),
            (i("video-params/w") > 0).then(|| format!("{}x{}", i("video-params/w"), i("video-params/h"))),
            f("video-bitrate").filter(|b| *b > 0.).map(mbps),
            f("container-fps").map(|x| format!("{x:.2} fps")),
        ]);
        let audio = join(vec![
            s("audio-codec-name").map(|c| c.to_uppercase()),
            (i("audio-params/samplerate") > 0).then(|| format!("{} Hz", i("audio-params/samplerate"))),
            (i("audio-params/channel-count") > 0).then(|| format!("{} 声道", i("audio-params/channel-count"))),
            f("audio-bitrate").filter(|b| *b > 0.).map(|b| format!("{:.0} kbps", b / 1e3)),
        ]);
        let decode = match s("hwdec-current").as_deref() {
            None | Some("no") | Some("") => "软件解码".to_string(),
            Some(x) => format!("硬件解码（{x}）"),
        };
        let fps = join(vec![f("container-fps").map(|x| format!("源 {x:.2}")), f("estimated-vf-fps").map(|x| format!("实际 {x:.2}"))]);
        let cache = f("demuxer-cache-duration").map(|d| format!("{d:.1} 秒{}", if s("paused-for-cache").as_deref() == Some("yes") { "（缓冲中）" } else { "" }));
        let host = s("path").and_then(|p| p.split("://").nth(1).and_then(|r| r.split('/').next()).map(str::to_string));
        let line = join(vec![Some(format!("{} / {}", i("playlist-pos") + 1, i("playlist-count"))), host]);
        [
            ("容器", s("file-format")),
            ("视频", Some(video).filter(|v| !v.is_empty())),
            ("解码", Some(decode)),
            ("音频", Some(audio).filter(|a| !a.is_empty())),
            ("帧率", Some(fps).filter(|v| !v.is_empty())),
            ("丢帧", Some(format!("解码 {} · 显示 {}", i("decoder-frame-drop-count"), i("frame-drop-count")))),
            ("缓冲", cache),
            ("网络速度", Some(mbps(i("cache-speed") as f64 * 8.))),
            ("线路", Some(line)),
        ]
        .into_iter()
        .filter_map(|(k, v)| Some((k, v?)))
        .collect()
    }

    /// 音量 0-100；静音单独控制，取消静音后恢复原音量
    /// 时移范围：(可回看的最早位置, 已缓存的最新位置, 当前位置)，同一条时间轴上的秒数；还没缓存时为 None
    pub fn timeshift(&self) -> Option<(f64, f64, f64)> {
        let pos = prop_f64(&self.mpv, "time-pos")?;
        let state: serde_json::Value = serde_json::from_str(&prop_str(&self.mpv, "demuxer-cache-state")?).ok()?;
        let r = state["seekable-ranges"].as_array()?.last()?;
        Some((r["start"].as_f64()?, r["end"].as_f64()?, pos))
    }

    /// 跳到时间轴上的 `t` 秒；调用方负责限制在 timeshift 范围内
    pub fn seek_to(&self, t: f64) {
        let (cmd, t, mode) = (cstr("seek"), cstr(&format!("{t:.3}")), cstr("absolute"));
        let mut args = [cmd.as_ptr(), t.as_ptr(), mode.as_ptr(), std::ptr::null()];
        unsafe { mpv_command(self.mpv.0, args.as_mut_ptr()) };
    }

    /// 暂停时 mpv 继续往缓存里下载，继续播放就是时移
    pub fn set_pause(&self, on: bool) {
        unsafe { mpv_set_property_string(self.mpv.0, cstr("pause").as_ptr(), cstr(if on { "yes" } else { "no" }).as_ptr()) };
    }

    pub fn paused(&self) -> bool {
        let mut v: c_int = 0;
        unsafe { mpv_get_property(self.mpv.0, cstr("pause").as_ptr(), FORMAT_FLAG, &mut v as *mut c_int as *mut c_void) };
        v != 0
    }

    pub fn set_volume(&self, volume: u8, muted: bool) {
        unsafe {
            mpv_set_property_string(self.mpv.0, cstr("volume").as_ptr(), cstr(&volume.min(100).to_string()).as_ptr());
            mpv_set_property_string(self.mpv.0, cstr("mute").as_ptr(), cstr(if muted { "yes" } else { "no" }).as_ptr());
        }
    }
}

pub(crate) fn prop(mpv: &Handle, name: &str) -> i64 {
    let mut v = 0i64;
    unsafe { mpv_get_property(mpv.0, cstr(name).as_ptr(), FORMAT_INT64, &mut v as *mut i64 as *mut c_void) };
    v
}

fn prop_f64(mpv: &Handle, name: &str) -> Option<f64> {
    let mut v = 0f64;
    (unsafe { mpv_get_property(mpv.0, cstr(name).as_ptr(), FORMAT_DOUBLE, &mut v as *mut f64 as *mut c_void) } >= 0).then_some(v)
}

fn prop_str(mpv: &Handle, name: &str) -> Option<String> {
    let p = unsafe { mpv_get_property_string(mpv.0, cstr(name).as_ptr()) };
    if p.is_null() {
        return None;
    }
    let v = unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned();
    unsafe { mpv_free(p as *mut c_void) };
    Some(v)
}

/// 输出尺寸：按视频比例缩到控件以内（不放大），宽度再受 `max_w` 限制。
fn fit(dw: i64, dh: i64, view: u64, max_w: i64) -> (i64, i64) {
    let (vw, vh) = ((view >> 32) as i64, (view & 0xFFFF_FFFF) as i64);
    let mut w = dw.min(max_w);
    if vw > 0 && vh > 0 {
        w = w.min(vw).min(vh * dw / dh);
    }
    let w = w.max(16);
    (w, (dh * w / dw).max(2))
}

/// 当前视频的输出尺寸；还没解出画面时返回 None。
pub(crate) fn target_size(mpv: &Handle, view: u64, max_w: i64) -> Option<(i64, i64)> {
    let (dw, dh) = (prop(mpv, "dwidth"), prop(mpv, "dheight"));
    (dw > 0 && dh > 0).then(|| fit(dw, dh, view, max_w))
}

struct Sw(*mut c_void);

impl Sw {
    fn new(mpv: *mut c_void) -> anyhow::Result<Self> {
        let mut ctx = std::ptr::null_mut();
        let sw = cstr("sw");
        let mut params = [RenderParam { kind: PARAM_API_TYPE, data: sw.as_ptr() as *mut c_void }, RenderParam { kind: 0, data: std::ptr::null_mut() }];
        check(unsafe { mpv_render_context_create(&mut ctx, mpv, params.as_mut_ptr()) }, "创建渲染上下文")?;
        Ok(Sw(ctx))
    }
}

impl Drop for Sw {
    fn drop(&mut self) {
        unsafe { mpv_render_context_free(self.0) };
    }
}

impl Backend for Sw {
    fn ctx(&self) -> *mut c_void {
        self.0
    }

    fn hwdec(&self) -> &'static str {
        "auto-copy-safe"
    }

    fn render(&mut self, mpv: &Handle, view: u64) -> Option<Frame> {
        let (tw, th) = target_size(mpv, view, SW_MAX_WIDTH)?;
        // 宽取 16 的倍数，stride 即为 64 的倍数（mpv 要求的快速路径）
        let w = (tw / 16 * 16).max(16);
        let h = (th * w / tw).max(1);
        let mut size = [w as c_int, h as c_int];
        let mut stride = w as usize * 4;
        let mut bgra = vec![0u8; stride * h as usize];
        let fmt = cstr("bgr0");
        let mut params = [
            RenderParam { kind: PARAM_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
            RenderParam { kind: PARAM_SW_FORMAT, data: fmt.as_ptr() as *mut c_void },
            RenderParam { kind: PARAM_SW_STRIDE, data: &mut stride as *mut usize as *mut c_void },
            RenderParam { kind: PARAM_SW_POINTER, data: bgra.as_mut_ptr() as *mut c_void },
            RenderParam { kind: 0, data: std::ptr::null_mut() },
        ];
        if unsafe { mpv_render_context_render(self.0, params.as_mut_ptr()) } < 0 {
            return None;
        }
        // bgr0 的第 4 字节是未定义值，GPUI 按 BGRA 混合，必须置为不透明
        bgra.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
        Some(Frame::Bgra { width: w as u32, height: h as u32, bgra })
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.wake.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        // 渲染上下文已在线程里释放，之后才能销毁 core
        unsafe { mpv_terminate_destroy(self.mpv.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用 lavfi 生成黑底、左上角白块的画面，不依赖网络；同时校验方向。
    #[test]
    fn renders_test_source() {
        let src = "av://lavfi:color=c=black:s=640x360:r=30,drawbox=x=0:y=0:w=320:h=90:color=white:t=fill";
        let (p, mut rx) = Player::new(&[src.into()], &[], true, 16).unwrap();
        let first = rx.blocking_recv().expect("frame");
        let stats = p.stats();
        assert!(stats.iter().any(|(k, v)| *k == "视频" && v.contains("640x360")), "{stats:?}");
        assert!(stats.iter().any(|(k, _)| *k == "线路"), "{stats:?}");
        match first {
            Frame::Bgra { width, height, bgra } => {
                assert_eq!((width, height), (640, 360));
                let px = |x: usize, y: usize| bgra[(y * 640 + x) * 4];
                assert!(px(10, 10) > 230 && px(10, 350) < 25, "方向或亮度不对");
                assert!(bgra.chunks(4).all(|p| p[3] == 255));
            }
            #[cfg(target_os = "macos")]
            Frame::Surface(s) => s.assert_test_source(640, 360),
        }
    }

    /// 设置里的缓冲区大小要同时决定前向缓冲和往回保留（可回看时长）的上限
    #[test]
    fn buffer_setting_reaches_mpv() {
        let src = "av://lavfi:color=c=black:s=64x64:r=10";
        for mb in [16, 64, 128] {
            let (p, _rx) = Player::new(&[src.into()], &[], true, mb).unwrap();
            let want = mb as i64 * 1024 * 1024;
            assert_eq!(prop(&p.mpv, "demuxer-max-bytes"), want, "前向缓冲 {mb}MB");
            assert_eq!(prop(&p.mpv, "demuxer-max-back-bytes"), want, "往回缓存 {mb}MB");
        }
    }

    #[test]
    fn fit_shrinks_to_view_keeping_aspect() {
        let view = |w: u64, h: u64| w << 32 | h;
        assert_eq!(fit(3840, 2160, 0, 3840), (3840, 2160)); // 尺寸未知：按源
        assert_eq!(fit(3840, 2160, view(800, 600), 3840), (800, 450)); // 宽受限
        assert_eq!(fit(3840, 2160, view(1600, 450), 3840), (800, 450)); // 高受限
        assert_eq!(fit(1280, 720, view(2560, 1440), 3840), (1280, 720)); // 不放大
        assert_eq!(fit(1920, 1080, view(2560, 1440), 1280), (1280, 720)); // max_w 封顶
    }

    /// 真实直播流（需要网络）：cargo test -p livo -- --ignored
    #[test]
    #[ignore]
    fn plays_live_streams() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        for site in livo_core::Site::all() {
            let pu = rt.block_on(async {
                let r = site.recommend_rooms(1).await?;
                let q = site.play_qualities(&r.items[0].room_id).await?;
                site.play_urls(&r.items[0].room_id, q[0].qn).await
            });
            let pu = pu.unwrap();
            let (_p, mut rx) = Player::new(&pu.urls, &pu.headers, true, 16).unwrap();
            let f = rt.block_on(async { tokio::time::timeout(std::time::Duration::from_secs(15), rx.recv()).await });
            assert!(matches!(f, Ok(Some(_))), "{} 15 秒内没有画面", site.name());
        }
    }

    /// 时移（需要网络）：播一会儿后能读到缓存范围、往回跳、暂停
    #[test]
    #[ignore]
    fn seeks_back_in_live_cache() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let wait = |s: u64| std::thread::sleep(std::time::Duration::from_secs(s));
        for site in livo_core::Site::all() {
            let pu = rt
                .block_on(async {
                    let r = site.recommend_rooms(1).await?;
                    let q = site.play_qualities(&r.items[0].room_id).await?;
                    site.play_urls(&r.items[0].room_id, q[0].qn).await
                })
                .unwrap();
            let (p, _rx) = Player::new(&pu.urls, &pu.headers, true, 16).unwrap();
            wait(15);
            let (start, end, pos) = p.timeshift().unwrap_or_else(|| panic!("{} 读不到缓存范围", site.name()));
            assert!(pos - start > 8., "{} 可回看不足 8 秒：{start:.1}..{end:.1} @ {pos:.1}", site.name());
            p.seek_to(pos - 6.);
            wait(2);
            let (_, _, after) = p.timeshift().unwrap();
            assert!(after < pos - 2., "{} 往回跳没生效：{pos:.1} -> {after:.1}", site.name());
            p.set_pause(true);
            assert!(p.paused());
            let (_, _, a) = p.timeshift().unwrap();
            wait(2);
            let (_, end2, b) = p.timeshift().unwrap();
            assert!((b - a).abs() < 0.1, "{} 暂停后进度还在走：{a:.1} -> {b:.1}", site.name());
            assert!(end2 > end, "{} 暂停时缓存没有继续增长", site.name());
            eprintln!("{} 时移正常：回看 {:.1} 秒", site.name(), pos - start);
        }
    }
}
