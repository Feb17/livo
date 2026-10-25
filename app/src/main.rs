// Windows 发行版不弹控制台窗口
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod icons;
mod img_http;
mod mpv;
mod settings;
mod store;
#[cfg(target_os = "macos")]
mod mpv_gl;

use gpui::{
    canvas, div, image_cache, img, relative, AnyView, ElementInputHandler, EntityInputHandler, UTF16Selection, UnderlineStyle, linear_color_stop, linear_gradient, point, prelude::*, px, rgb, rgba, size, App, Application, Bounds, Context, Entity, FocusHandle, FontWeight,
    HighlightStyle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, Pixels, RenderImage, RetainAllImageCache, Rgba, ScrollHandle, SharedString, StyleRefinement, StyledText, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use icons::icon;
use simple_live_core::{danmaku, Category, LiveMessage, PlayQuality, RoomDetail, RoomItem, RoomPage, Site, SubCategory};
use std::cell::Cell;
use std::ops::Range;
use simple_live_core::bilibili::BiliBili;
use store::{DanmakuStyle, Follow, History, Store, Theme, HOME_PAGES, KEY_ACTIONS};
use std::rc::Rc;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

// 浅色/深色两套主题色，由根视图每帧按设置（或系统外观）切换；强调色跟随当前平台
static LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn themed(dark: u32, light: u32) -> u32 {
    if LIGHT.load(Ordering::Relaxed) {
        light
    } else {
        dark
    }
}
fn bg() -> u32 {
    themed(0x0e1014, 0xf4f5f7)
}
fn sidebar() -> u32 {
    themed(0x13161c, 0xffffff)
}
fn surface() -> u32 {
    themed(0x1a1e26, 0xffffff)
}
fn hover() -> u32 {
    themed(0x232834, 0xeceef2)
}
fn border() -> u32 {
    themed(0x262b36, 0xe1e4ea)
}
fn text() -> u32 {
    themed(0xe9ecf1, 0x1b1e24)
}
fn muted() -> u32 {
    themed(0x8a93a3, 0x5d6573)
}
fn subtle() -> u32 {
    themed(0x5b6372, 0x9aa1ad)
}
const ACCENTS: [u32; 3] = [0xfb7299, 0xff7a1a, 0xffb020];
const NAME_COLORS: [u32; 6] = [0x7aa2f7, 0x9ece6a, 0xe0af68, 0xbb9af7, 0x7dcfff, 0xf7768e];
const SIDEBAR_W: f32 = 208.;
const CHAT_W: f32 = 320.;
const PAD: f32 = 24.;
const GAP: f32 = 16.;
const MIN_CARD: f32 = 220.;
const MAX_DANMAKU: usize = 200;
/// 滚动弹幕同一弹道前后两条的间距
const DM_GAP: f32 = 24.;

fn rt() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("tokio runtime"))
}

fn tint(color: u32, alpha: u8) -> Rgba {
    rgba(color << 8 | alpha as u32)
}

/// 12345 -> "1.2万"
fn fmt_count(n: i64) -> String {
    match n {
        n if n >= 100_000_000 => format!("{:.1}亿", n as f64 / 1e8),
        n if n >= 10_000 => format!("{:.1}万", n as f64 / 1e4),
        n => n.to_string(),
    }
}

/// 同一个用户名总是同一个颜色
fn name_color(name: &str) -> u32 {
    NAME_COLORS[name.bytes().fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % NAME_COLORS.len()]
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Recommend,
    Category,
    Search,
    Follow,
    History,
    Settings,
}

impl Tab {
    /// 内容随平台变化的页面
    fn is_site_page(self) -> bool {
        matches!(self, Tab::Recommend | Tab::Category | Tab::Search)
    }

    /// 主页设置里 HOME_PAGES 的 id
    fn from_id(id: &str) -> Option<Tab> {
        Some(match id {
            "recommend" => Tab::Recommend,
            "follow" => Tab::Follow,
            "category" => Tab::Category,
            "search" => Tab::Search,
            _ => return None,
        })
    }
}

/// 扫码登录进行中的状态
struct Login {
    key: String,
    /// 二维码模块，按行展开；true 为深色
    qr: Vec<bool>,
    width: usize,
    status: &'static str,
}

/// 房间列表的来源，翻页时按它继续加载
#[derive(Clone)]
enum Source {
    Recommend,
    Category(SubCategory),
    Search(String),
}

struct Msg {
    user: SharedString,
    text: SharedString,
    color: u32,
    price: Option<i64>,
}

struct Room {
    id: u64,
    detail: RoomDetail,
    chat: Entity<Chat>,
    online: i64,
    qualities: Vec<PlayQuality>,
    quality_idx: usize,
    /// 当前线路；播放时从它开始，失败了 mpv 再往后切
    line_idx: usize,
    urls: Vec<String>,
    headers: Vec<(String, String)>,
    player: Option<mpv::Player>,
    danmaku_task: tokio::task::JoinHandle<()>,
}

impl Drop for Room {
    fn drop(&mut self) {
        self.danmaku_task.abort();
    }
}

/// 播放器右侧弹出的选择面板
#[derive(Clone, Copy, PartialEq)]
enum Menu {
    Quality,
    Line,
    Danmaku,
}

/// 关注页的一项：本地记录 + 刚查到的开播状态
#[derive(Clone)]
struct FollowEntry {
    follow: Follow,
    /// None 表示还在查询或查询失败
    live: Option<bool>,
    online: i64,
}

/// 一条滚动弹幕；位置按出发时间实时算
struct Bullet {
    text: SharedString,
    color: u32,
    lane: usize,
    start: Instant,
    width: f32,
}

/// 当前视频帧：软件渲染的图要进纹理图集，macOS GPU 渲染直接是 IOSurface。
#[derive(Clone)]
enum VideoFrame {
    Image(Arc<RenderImage>),
    #[cfg(target_os = "macos")]
    Surface(core_video::pixel_buffer::CVPixelBuffer),
}

/// 视频画面单独一个 entity：每帧只重绘它，不牵动弹幕和其他界面。
#[derive(Default)]
struct Video {
    frame: Option<VideoFrame>,
    /// 播放器的控件尺寸回传通道；None 表示没在播放
    view: Option<Arc<AtomicU64>>,
    /// 视频上的滚动弹幕开关
    overlay: bool,
    style: DanmakuStyle,
    bullets: Vec<Bullet>,
    /// 每条弹道下一次空出来的时刻
    lanes: Vec<Instant>,
    /// 控件的逻辑尺寸（宽, 高），绘制时回填，供弹幕计算位置
    size: Rc<Cell<(f32, f32)>>,
    /// 直播设置里的画面尺寸（见 fit_of）
    fit: u8,
}

impl Video {
    /// 发射一条弹幕；没有空弹道就丢掉，避免重叠
    fn shoot(&mut self, text: String, color: u32) {
        let (w, h) = self.size.get();
        if !self.overlay || self.view.is_none() || w <= 0. {
            return;
        }
        let now = Instant::now();
        let st = self.style;
        let lanes = ((h * st.area) / (st.font * 1.5)).floor().max(1.) as usize;
        self.lanes.resize(lanes, now);
        let Some(lane) = self.lanes.iter().position(|t| *t <= now) else { return };
        // shortcut: 按字符估算宽度（中日韩 1em，其余 0.55em），够用来排弹道；要精确再接文字测量
        let width = text.chars().map(|c| if c as u32 >= 0x2E80 { st.font } else { st.font * 0.55 }).sum::<f32>();
        // 匀速前进，前一条尾巴离开右边缘一段距离后这条弹道才可用，所以不会追尾
        self.lanes[lane] = now + Duration::from_secs_f32((width + DM_GAP) / (w / st.secs));
        self.bullets.push(Bullet { text: text.into(), color, lane, start: now, width });
    }

    fn render_bullets(&mut self) -> impl IntoElement {
        let (w, _) = self.size.get();
        let st = self.style;
        let (now, speed) = (Instant::now(), w / st.secs);
        let x_of = |b: &Bullet| w - now.duration_since(b.start).as_secs_f32() * speed;
        self.bullets.retain(|b| x_of(b) + b.width > 0.);
        // 没有描边能力，先画一层偏移的半透明黑字当阴影
        div().absolute().top_0().left_0().size_full().overflow_hidden().opacity(st.opacity).children(self.bullets.iter().flat_map(|b| {
            let (x, y) = (x_of(b), 8. + b.lane as f32 * st.font * 1.5);
            [(x + 1., y + 1., tint(0x000000, 0xb0)), (x, y, rgb(b.color))]
                .map(|(x, y, c)| div().absolute().left(px(x)).top(px(y)).whitespace_nowrap().text_size(px(st.font)).font_weight(FontWeight::MEDIUM).text_color(c).child(b.text.clone()))
        }))
    }
}

impl Render for Video {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut screen = div().relative().size_full().bg(rgb(0x000000)).flex().items_center().justify_center();
        if let Some(view) = self.view.clone() {
            let size = self.size.clone();
            // 把控件的物理像素尺寸告诉渲染线程，输出不超过实际显示大小
            screen = screen.child(
                canvas(
                    move |b, window, _| {
                        let (lw, lh) = (f32::from(b.size.width), f32::from(b.size.height));
                        size.set((lw, lh));
                        let sf = window.scale_factor();
                        let (w, h) = ((lw * sf) as u64, (lh * sf) as u64);
                        view.store(w << 32 | h, Ordering::Relaxed);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );
        }
        let screen = match (&self.view, &self.frame) {
            (Some(_), Some(VideoFrame::Image(f))) => screen.child(img(f.clone()).size_full().object_fit(fit_of(self.fit))),
            #[cfg(target_os = "macos")]
            (Some(_), Some(VideoFrame::Surface(pb))) => screen.child(gpui::surface(pb.clone()).size_full().object_fit(fit_of(self.fit))),
            (Some(_), None) => screen.text_color(rgb(muted())).child("正在缓冲…"),
            (None, _) => screen.text_color(rgb(muted())).child("已暂停"),
        };
        // 视频帧每到一帧就重绘一次，弹幕位置随之前进
        let show = self.overlay && self.view.is_some();
        screen.children(show.then(|| self.render_bullets()))
    }
}

struct SimpleLive {
    sites: Vec<Arc<Site>>,
    site_idx: usize,
    focus: FocusHandle,
    tab: Tab,
    /// 最近一次停留的平台页面（推荐/分类/搜索）
    site_tab: Tab,
    source: Source,
    rooms: Vec<RoomItem>,
    page: u32,
    has_more: bool,
    cats: Vec<Category>,
    cat_idx: usize,
    /// 分类页里选中的子分类；None 时显示分类网格
    sub: Option<SubCategory>,
    keyword: String,
    loading: bool,
    error: Option<String>,
    room: Option<Room>,
    video: Entity<Video>,
    menu: Option<Menu>,
    show_danmaku: bool,
    volume: u8,
    muted: bool,
    /// 各滑块的位置，绘制时回填，用来把鼠标坐标换算成数值：0 音量，1-4 弹幕设置
    sliders: [Rc<Cell<gpui::Bounds<Pixels>>>; 5],
    store: Store,
    follow_list: Vec<FollowEntry>,
    /// 搜索框里输入法正在组字的部分（字节范围）
    marked: Option<Range<usize>>,
    /// 播放页设置里正在录入快捷键的动作
    capturing: Option<&'static str>,
    show_chat: bool,
    /// 进入直播间后等下一帧拿到 window 再全屏
    pending_fullscreen: bool,
    /// 定时关闭的代号：离开或重进直播间时作废旧的倒计时
    exit_gen: u64,
    last_follow_refresh: Instant,
    login: Option<Login>,
    /// 递增代号：切换页面/房间后丢弃过期的异步结果与弹幕
    epoch: u64,
    /// 浏览页的封面/分类图。GPUI 默认的图片缓存只增不减（内存 + 显存各一份），
    /// 所以浏览页用自己的缓存，换页面或进直播间时整个换掉，旧图随之释放
    images: Entity<RetainAllImageCache>,
}

impl SimpleLive {
    fn new(cx: &mut Context<Self>) -> Self {
        let store = Store::load();
        let mut sites: Vec<Arc<Site>> = Site::all()
            .into_iter()
            .map(|s| match s {
                // 已登录 B站就带上 Cookie
                Site::BiliBili(_) if !store.bili.cookie.is_empty() => Site::BiliBili(BiliBili::with_cookie(store.bili.cookie.clone(), store.bili.uid)),
                s => s,
            })
            .map(Arc::new)
            .collect();
        // 主页设置里的平台顺序，没排过的放后面
        sites.sort_by_key(|s| store.site_sort.iter().position(|id| id == s.id()).unwrap_or(usize::MAX));
        let fit = store.live.fit;
        let mut s = Self {
            sites,
            site_idx: 0,
            focus: cx.focus_handle(),
            tab: Tab::Recommend,
            site_tab: Tab::Recommend,
            source: Source::Recommend,
            rooms: vec![],
            page: 0,
            has_more: false,
            cats: vec![],
            cat_idx: 0,
            sub: None,
            keyword: String::new(),
            loading: false,
            error: None,
            room: None,
            video: cx.new(|_| Video { overlay: store.danmaku.enabled, style: store.danmaku, fit, ..Default::default() }),
            menu: None,
            show_danmaku: store.danmaku.enabled,
            volume: 100,
            muted: false,
            sliders: Default::default(),
            store,
            follow_list: vec![],
            images: RetainAllImageCache::new(cx),
            marked: None,
            capturing: None,
            show_chat: true,
            pending_fullscreen: false,
            exit_gen: 0,
            last_follow_refresh: Instant::now(),
            login: None,
            epoch: 0,
        };
        // 主页设置里排第一的页面是启动页
        let first = s.store.home_sort.iter().find_map(|id| Tab::from_id(id)).unwrap_or(Tab::Recommend);
        s.site_tab = s.store.home_sort.iter().filter_map(|id| Tab::from_id(id)).find(|t| t.is_site_page()).unwrap_or(Tab::Recommend);
        s.go(first, cx);
        // 关注设置：后台按间隔刷新开播状态，每分钟检查一次是否到点
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(60)).await;
            let alive = this.update(cx, |this, cx| {
                let f = this.store.follow;
                if f.auto_refresh && this.last_follow_refresh.elapsed() >= Duration::from_secs(f.interval_min as u64 * 60) {
                    this.refresh_follow_status(cx);
                }
            });
            if alive.is_err() {
                break;
            }
        })
        .detach();
        s
    }

    fn site(&self) -> Arc<Site> {
        self.sites[self.site_idx].clone()
    }

    fn accent(&self) -> u32 {
        ACCENTS[self.site_idx % ACCENTS.len()]
    }

    fn switch_site(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.site_idx = idx;
        self.room = None;
        self.cats.clear();
        self.cat_idx = 0;
        self.sub = None;
        // 关注/观看记录/设置不分平台，点平台时回到上次浏览的平台页面
        let tab = if self.tab.is_site_page() { self.tab } else { self.site_tab };
        self.go(tab, cx);
    }

    /// 切到某个页面（侧边栏导航）
    fn go(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.room = None;
        self.images = RetainAllImageCache::new(cx);
        self.tab = tab;
        if tab.is_site_page() {
            self.site_tab = tab;
        }
        match tab {
            Tab::Recommend => self.load_rooms(Source::Recommend, cx),
            Tab::Category => {
                self.sub = None;
                if self.cats.is_empty() {
                    let site = self.site();
                    self.fetch(cx, async move { site.categories().await }, |this, c, _| this.cats = c);
                }
            }
            Tab::Search => match self.keyword.trim() {
                "" => self.rooms.clear(),
                kw => self.load_rooms(Source::Search(kw.to_string()), cx),
            },
            Tab::Follow => self.load_follows(cx),
            Tab::History | Tab::Settings => {}
        }
        cx.notify();
    }

    /// 播放/停止切换；帧循环在播放器销毁（发送端关闭）后把最后一帧移出纹理图集。
    fn play(&mut self, cx: &mut Context<Self>) {
        let (volume, muted, live) = (self.volume, self.muted, self.store.live.clone());
        let Some(room) = self.room.as_mut() else { return };
        if room.player.take().is_some() {
            self.video.update(cx, |v, cx| {
                v.view = None;
                v.bullets.clear();
                cx.notify()
            });
            return cx.notify();
        }
        // 从选中的线路开始，其余线路排在后面做失败兜底
        let mut urls = room.urls.clone();
        let start = room.line_idx.min(urls.len().saturating_sub(1));
        urls.rotate_left(start);
        if live.https {
            urls.iter_mut().for_each(|u| *u = u.replacen("http://", "https://", 1));
        }
        match mpv::Player::new(&urls, &room.headers, live.hwdec, live.buffer_mb) {
            Ok((p, mut rx)) => {
                p.set_volume(volume, muted);
                let view = p.view.clone();
                room.player = Some(p);
                self.video.update(cx, |v, cx| {
                    v.view = Some(view.clone());
                    v.bullets.clear();
                    cx.notify()
                });
                let video = self.video.downgrade();
                cx.spawn(async move |_, cx| {
                    // 只有进了图集的图需要手动移除
                    let mut last: Option<Arc<RenderImage>> = None;
                    while let Some(f) = rx.recv().await {
                        let frame = match f {
                            mpv::Frame::Bgra { width, height, bgra } => {
                                let Some(buf) = image::RgbaImage::from_raw(width, height, bgra) else { continue };
                                VideoFrame::Image(Arc::new(RenderImage::new(vec![image::Frame::new(buf)])))
                            }
                            #[cfg(target_os = "macos")]
                            mpv::Frame::Surface(s) => VideoFrame::Surface(s.0),
                        };
                        let alive = video.update(cx, |v, cx| {
                            let old = last.take();
                            if let VideoFrame::Image(img) = &frame {
                                last = Some(img.clone());
                            }
                            v.frame = Some(frame);
                            if let Some(old) = old {
                                cx.drop_image(old, None);
                            }
                            cx.notify();
                        });
                        if alive.is_err() {
                            return;
                        }
                    }
                    video
                        .update(cx, |v, cx| {
                            if let Some(old) = last {
                                cx.drop_image(old, None);
                            }
                            // 只清自己这一轮的画面；已经开始新一轮播放就不动
                            if v.view.as_ref().is_none_or(|cur| Arc::ptr_eq(cur, &view)) {
                                v.frame = None;
                                v.view = None;
                            }
                            cx.notify();
                        })
                        .ok();
                })
                .detach();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        cx.notify();
    }

    /// 在 tokio 上跑 core 的请求，结果回到 UI 线程；epoch 过期则丢弃。
    fn fetch<T: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        job: impl std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
        done: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        self.epoch += 1;
        let epoch = self.epoch;
        self.loading = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = rt().spawn(job).await;
            this.update(cx, |this, cx| {
                if this.epoch != epoch {
                    return;
                }
                this.loading = false;
                match r {
                    Ok(Ok(v)) => done(this, v, cx),
                    Ok(Err(e)) => this.error = Some(e.to_string()),
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load_rooms(&mut self, source: Source, cx: &mut Context<Self>) {
        self.images = RetainAllImageCache::new(cx);
        self.source = source;
        self.rooms.clear();
        self.page = 0;
        self.has_more = false;
        self.load_more(cx);
    }

    fn load_more(&mut self, cx: &mut Context<Self>) {
        let (site, page, source) = (self.site(), self.page + 1, self.source.clone());
        let job = async move {
            match source {
                Source::Recommend => site.recommend_rooms(page).await,
                Source::Category(sub) => site.category_rooms(&sub, page).await,
                Source::Search(kw) => site.search_rooms(&kw, page).await,
            }
        };
        self.fetch(cx, job, move |this, p: RoomPage, _| {
            this.page = page;
            this.has_more = p.has_more && !p.items.is_empty();
            // 分页接口偶尔重叠，去重以免卡片 id 冲突
            for r in p.items {
                if !this.rooms.iter().any(|x| x.room_id == r.room_id) {
                    this.rooms.push(r);
                }
            }
        });
    }

    fn open_sub(&mut self, sub: SubCategory, cx: &mut Context<Self>) {
        self.sub = Some(sub.clone());
        self.load_rooms(Source::Category(sub), cx);
    }

    fn open_room(&mut self, room_id: String, cx: &mut Context<Self>) {
        let site = self.site();
        let level = self.store.live.quality_level;
        let memory = self.store.live.quality_memory.get(site.id()).cloned();
        let job = async move {
            let detail = site.room_detail(&room_id).await?;
            let q = site.play_qualities(&detail.room_id).await?;
            let idx = default_quality(&q, level, memory.as_deref());
            let urls = site.play_urls(&detail.room_id, q[idx].qn).await?;
            anyhow::ensure!(!urls.urls.is_empty(), "没有可用的播放地址");
            Ok((detail, q, idx, urls))
        };
        self.fetch(cx, job, |this, (detail, qualities, quality_idx, urls), cx| {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let site = this.site().id().to_string();
            this.store.record(History { site, room_id: detail.room_id.clone(), user_name: detail.user_name.clone(), face: detail.user_avatar.clone(), title: detail.title.clone(), cover: detail.cover.clone(), time: now });
            this.store.save();
            // 直播间里看不到浏览页，先把它的图片放掉；退出时重新加载
            this.images = RetainAllImageCache::new(cx);
            this.pending_fullscreen = this.store.live.auto_fullscreen;
            // 直播设置/弹幕设置在每次进入直播间时生效
            let (fit, dm) = (this.store.live.fit, this.store.danmaku.enabled);
            this.show_danmaku = dm;
            this.video.update(cx, |v, _| {
                v.fit = fit;
                v.overlay = dm;
            });
            this.start_auto_exit(cx);
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let args = detail.danmaku.clone();
            let danmaku_task = rt().spawn(async move {
                let _ = danmaku::run(args, tx).await;
            });
            let id = this.epoch;
            let chat = cx.new(|_| Chat { messages: vec![], scroll: ScrollHandle::new(), accent: this.accent() });
            this.menu = None;
            this.room = Some(Room { id, online: detail.online, detail, chat, qualities, quality_idx, line_idx: 0, urls: urls.urls, headers: urls.headers, player: None, danmaku_task });
            this.play(cx);
            cx.spawn(async move |this, cx| {
                while let Some(m) = rx.recv().await {
                    let alive = this.update(cx, |this, cx| {
                        let Some(room) = this.room.as_mut().filter(|r| r.id == id) else { return false };
                        match m {
                            LiveMessage::Online(n) => {
                                room.online = n;
                                cx.notify();
                            }
                            // 弹幕只通知弹幕面板，不重绘整个页面；滚动弹幕随下一帧视频画出来
                            m => {
                                if let LiveMessage::Chat { text, color: (r, g, b), .. } = &m {
                                    let c = (*r as u32) << 16 | (*g as u32) << 8 | *b as u32;
                                    this.video.update(cx, |v, _| v.shoot(text.clone(), c));
                                }
                                room.chat.update(cx, |c, cx| c.push(m, cx))
                            }
                        }
                        true
                    });
                    if !matches!(alive, Ok(true)) {
                        break;
                    }
                }
            })
            .detach();
        });
    }

    /// 定时关闭：从进入直播间开始倒计时，到点退出应用；离开直播间即取消
    fn start_auto_exit(&mut self, cx: &mut Context<Self>) {
        self.exit_gen += 1;
        let (gen, auto) = (self.exit_gen, self.store.auto_exit);
        if !auto.enabled {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(auto.minutes as u64 * 60)).await;
            this.update(cx, |this, cx| {
                if this.exit_gen == gen && this.room.is_some() && this.store.auto_exit.enabled {
                    cx.quit();
                }
            })
            .ok();
        })
        .detach();
    }

    fn close_room(&mut self, cx: &mut Context<Self>) {
        self.exit_gen += 1;
        self.room = None;
        self.video.update(cx, |v, cx| {
            v.view = None;
            cx.notify()
        });
        cx.notify();
    }

    /// 换线路：从这条开始重新播放
    fn choose_line(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.menu = None;
        let Some(room) = self.room.as_mut() else { return };
        room.line_idx = idx;
        room.player = None;
        self.play(cx);
    }

    /// 换清晰度：重新取这一档的播放地址；刷新就是按当前清晰度再取一次
    fn choose_quality(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.menu = None;
        let Some(room) = self.room.as_ref() else { return };
        let Some(q) = room.qualities.get(idx) else { return };
        let (site, room_id, qn, id) = (self.site(), room.detail.room_id.clone(), q.qn, room.id);
        self.fetch(cx, async move { site.play_urls(&room_id, qn).await }, move |this, urls, cx| {
            let Some(room) = this.room.as_mut().filter(|r| r.id == id) else { return };
            room.quality_idx = idx;
            // 直播设置“记住上次”：按平台记清晰度名称
            if let Some(q) = room.qualities.get(idx) {
                this.store.live.quality_memory.insert(this.sites[this.site_idx].id().to_string(), q.name.clone());
                this.store.save();
            }
            room.line_idx = 0;
            room.urls = urls.urls;
            room.headers = urls.headers;
            room.player = None;
            this.play(cx);
        });
    }

    fn toggle_danmaku(&mut self, cx: &mut Context<Self>) {
        self.show_danmaku = !self.show_danmaku;
        let on = self.show_danmaku;
        self.video.update(cx, |v, cx| {
            v.overlay = on;
            v.bullets.clear();
            cx.notify()
        });
        cx.notify();
    }

    fn set_volume(&mut self, volume: u8, muted: bool, cx: &mut Context<Self>) {
        self.volume = volume.min(100);
        self.muted = muted;
        if let Some(p) = self.room.as_ref().and_then(|r| r.player.as_ref()) {
            p.set_volume(self.volume, self.muted);
        }
        cx.notify();
    }

    /// 鼠标横坐标在第 idx 个滑块上的比例
    fn slider_frac(&self, idx: usize, x: Pixels) -> f32 {
        let b = self.sliders[idx].get();
        (f32::from(x - b.left()) / f32::from(b.size.width).max(1.)).clamp(0., 1.)
    }

    fn set_danmaku_style(&mut self, style: DanmakuStyle, cx: &mut Context<Self>) {
        self.store.danmaku = style;
        self.store.save();
        self.video.update(cx, |v, cx| {
            v.style = style;
            cx.notify()
        });
        cx.notify();
    }

    fn site_index(&self, id: &str) -> Option<usize> {
        self.sites.iter().position(|s| s.id() == id)
    }

    /// 关注页：先按本地记录列出来，再并发查每个房间的开播状态
    fn load_follows(&mut self, cx: &mut Context<Self>) {
        // 保留已经查到的开播状态，只按本地记录重排
        let old = std::mem::take(&mut self.follow_list);
        self.follow_list = self
            .store
            .follows
            .iter()
            .map(|f| old.iter().find(|e| e.follow.site == f.site && e.follow.room_id == f.room_id).map_or(FollowEntry { follow: f.clone(), live: None, online: 0 }, |e| FollowEntry { follow: f.clone(), ..*e }))
            .collect();
        self.refresh_follow_status(cx);
    }

    /// 并发查每个关注房间的开播状态（线程数见关注设置）；不占用页面的 loading/epoch
    fn refresh_follow_status(&mut self, cx: &mut Context<Self>) {
        self.last_follow_refresh = Instant::now();
        let jobs: Vec<_> = self.store.follows.iter().map(|f| (self.site_index(&f.site).map(|i| self.sites[i].clone()), f.site.clone(), f.room_id.clone())).collect();
        let threads = self.store.follow.threads.max(1) as usize;
        cx.spawn(async move |this, cx| {
            let done = rt()
                .spawn(async move {
                    let limit = Arc::new(tokio::sync::Semaphore::new(threads));
                    let mut set = tokio::task::JoinSet::new();
                    for (site, site_id, room_id) in jobs {
                        let limit = limit.clone();
                        set.spawn(async move {
                            let _permit = limit.acquire_owned().await;
                            let detail = match site {
                                Some(s) => s.room_detail(&room_id).await.ok(),
                                None => None,
                            };
                            (site_id, room_id, detail)
                        });
                    }
                    let mut out = vec![];
                    while let Some(r) = set.join_next().await {
                        out.extend(r.ok());
                    }
                    out
                })
                .await
                .unwrap_or_default();
            this.update(cx, |this, cx| {
                for (site, room_id, detail) in done {
                    let Some(d) = detail else { continue };
                    // 按 (平台, 房间号) 回写，查询期间取消的关注不会被写回来
                    let refresh = |f: &mut Follow| {
                        f.title = d.title.clone();
                        f.cover = d.cover.clone();
                        f.face = d.user_avatar.clone();
                        f.user_name = d.user_name.clone();
                    };
                    if let Some(f) = this.store.follows.iter_mut().find(|f| f.site == site && f.room_id == room_id) {
                        refresh(f);
                    }
                    if let Some(e) = this.follow_list.iter_mut().find(|e| e.follow.site == site && e.follow.room_id == room_id) {
                        refresh(&mut e.follow);
                        e.live = Some(d.live);
                        e.online = d.online;
                    }
                }
                this.store.save();
                // 正在直播的排前面，其余保持关注顺序
                this.follow_list.sort_by_key(|e| e.live != Some(true));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        let Some(room) = self.room.as_ref() else { return };
        let (site, d) = (self.site().id().to_string(), &room.detail);
        if self.store.is_followed(&site, &d.room_id) {
            self.store.follows.retain(|f| !(f.site == site && f.room_id == d.room_id));
        } else {
            self.store.follows.push(Follow { site, room_id: d.room_id.clone(), user_name: d.user_name.clone(), face: d.user_avatar.clone(), title: d.title.clone(), cover: d.cover.clone() });
        }
        self.store.save();
        cx.notify();
    }

    fn open_follow(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(e) = self.follow_list.get(idx) else { return };
        let (site, room_id) = (e.follow.site.clone(), e.follow.room_id.clone());
        self.open_on(&site, room_id, cx);
    }

    /// 打开某平台的房间（关注、观看记录都可能跨平台）
    fn open_on(&mut self, site_id: &str, room_id: String, cx: &mut Context<Self>) {
        let Some(site) = self.site_index(site_id) else { return };
        if site != self.site_idx {
            self.site_idx = site;
            self.cats.clear();
            self.sub = None;
        }
        self.open_room(room_id, cx);
    }

    /// 播放页设置的快捷键动作
    fn run_action(&mut self, action: &str, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            "fullscreen" => window.toggle_fullscreen(),
            "danmaku" => self.toggle_danmaku(cx),
            "mute" => self.set_volume(self.volume, !self.muted, cx),
            "refresh" => {
                let idx = self.room.as_ref().map_or(0, |r| r.quality_idx);
                self.choose_quality(idx, cx)
            }
            "chat" => {
                self.show_chat = !self.show_chat;
                cx.notify()
            }
            "volume_up" => self.set_volume(self.volume.saturating_add(5), false, cx),
            "volume_down" => self.set_volume(self.volume.saturating_sub(5), false, cx),
            "pause" => self.play(cx),
            _ => {}
        }
    }

    /// 输入法回调给的 UTF-16 范围换成关键词里的字节范围；没给就替换正在组字的部分，否则追加到末尾
    fn input_range(&self, range: Option<Range<usize>>) -> Range<usize> {
        range.map(|r| utf16_to_bytes(&self.keyword, r)).or(self.marked.clone()).unwrap_or(self.keyword.len()..self.keyword.len())
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        // 正在录入快捷键：这一下按键就是新绑定（Esc 取消），并且不要再交给搜索框
        if let Some(action) = self.capturing.take() {
            if key != "escape" {
                self.store.keys.insert(action.to_string(), key.to_string());
                self.store.save();
            }
            cx.stop_propagation();
            return cx.notify();
        }
        if self.room.is_some() {
            match key {
                "escape" if self.menu.is_some() => {
                    self.menu = None;
                    cx.notify()
                }
                "escape" if window.is_fullscreen() => window.toggle_fullscreen(),
                "escape" => self.close_room(cx),
                _ => {
                    if let Some((action, ..)) = KEY_ACTIONS.iter().find(|(a, ..)| self.store.key_for(a) == key) {
                        self.run_action(action, window, cx)
                    }
                }
            }
            return;
        }
        // 列表页里直接打字就是搜索，回车出结果；组字中的回车/退格由输入法先处理
        match key {
            "enter" => return self.go(Tab::Search, cx),
            "escape" => {
                self.keyword.clear();
                self.marked = None;
            }
            "backspace" => {
                self.keyword.pop();
            }
            // 文字输入（含输入法）走搜索框注册的 EntityInputHandler
            _ => return,
        }
        cx.notify();
    }
}

/// 弹幕面板单独一个 entity 并按缓存渲染：视频每帧重绘时直接复用上一帧的排版。
struct Chat {
    messages: Vec<Msg>,
    scroll: ScrollHandle,
    accent: u32,
}

impl Chat {
    fn push(&mut self, m: LiveMessage, cx: &mut Context<Self>) {
        let msg = match m {
            LiveMessage::Online(_) => return,
            LiveMessage::Chat { user, text, color: (r, g, b), .. } => {
                Msg { color: name_color(&user), user: user.into(), text: text.into(), price: None }.with_text_color((r as u32) << 16 | (g as u32) << 8 | b as u32)
            }
            LiveMessage::SuperChat { user, text, price } => Msg { color: name_color(&user), user: user.into(), text: text.into(), price: Some(price) },
        };
        // 用户往上翻看历史时不强行拉回底部
        let at_bottom = -self.scroll.offset().y >= self.scroll.max_offset().height - px(24.);
        self.messages.push(msg);
        if self.messages.len() > MAX_DANMAKU {
            self.messages.drain(..MAX_DANMAKU / 4);
        }
        if at_bottom {
            self.scroll.scroll_to_bottom();
        }
        cx.notify();
    }
}

impl Render for Chat {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let accent = self.accent;
        let messages = self.messages.iter().map(|m| {
            let line = format!("{}  {}", m.user, m.text);
            let name = 0..m.user.len();
            let styled = StyledText::new(line).with_highlights([(name, HighlightStyle { color: Some(rgb(m.color).into()), font_weight: Some(FontWeight::MEDIUM), ..Default::default() })]);
            match m.price {
                Some(price) => div()
                    .p_2p5()
                    .rounded_lg()
                    .bg(tint(accent, 0x1c))
                    .border_1()
                    .border_color(tint(accent, 0x55))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().flex().justify_between().text_xs().child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(accent)).child(m.user.clone())).child(div().px_1p5().rounded_sm().bg(rgb(accent)).text_color(rgb(0x111111)).child(format!("¥{price}"))))
                    .child(div().text_sm().child(m.text.clone())),
                None => div().px_1().py_0p5().text_sm().text_color(rgb(text())).child(styled),
            }
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .rounded_xl()
            .bg(rgb(surface()))
            .border_1()
            .border_color(rgb(border()))
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .px_4()
                    .py_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(border()))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("弹幕"))
                    .child(div().flex().items_center().gap_1p5().text_xs().text_color(rgb(muted())).child(div().size(px(6.)).rounded_full().bg(rgb(0x2ed573))).child("实时")),
            )
            .child(if self.messages.is_empty() {
                div().id("chat").flex_1().flex().items_center().justify_center().text_sm().text_color(rgb(subtle())).child("等待弹幕…")
            } else {
                div().id("chat").flex_1().overflow_y_scroll().track_scroll(&self.scroll).p_3().flex().flex_col().gap_1().children(messages)
            })
    }
}

impl Msg {
    /// 白色弹幕用名字色，彩色弹幕保留平台下发的颜色
    fn with_text_color(mut self, c: u32) -> Self {
        if c != 0xffffff {
            self.color = c;
        }
        self
    }
}

/// 按直播设置选默认清晰度（同原项目）：0 最低，1 中等，2 最高，3 记住上次（找不到退回中等）
fn default_quality(q: &[PlayQuality], level: u8, memory: Option<&str>) -> usize {
    let middle = q.len() / 2;
    match level {
        0 => q.len().saturating_sub(1),
        2 => 0,
        3 => memory.and_then(|m| q.iter().position(|x| x.name == m)).unwrap_or(middle),
        _ => middle,
    }
}

/// 直播设置的画面尺寸：适应 / 填充 / 拉伸
fn fit_of(fit: u8) -> ObjectFit {
    match fit {
        1 => ObjectFit::Cover,
        2 => ObjectFit::Fill,
        _ => ObjectFit::Contain,
    }
}

/// 侧边栏导航项
fn nav_item(id: impl Into<SharedString>, label: impl Into<SharedString>, active: bool, accent: u32) -> gpui::Stateful<gpui::Div> {
    let item = div().id(id.into()).px_3().py_2().rounded_lg().cursor_pointer().flex().items_center().gap_2().text_sm();
    if active {
        item.bg(tint(accent, 0x22)).text_color(rgb(accent)).font_weight(FontWeight::MEDIUM)
    } else {
        item.text_color(rgb(muted())).hover(|s| s.bg(rgb(hover())).text_color(rgb(text())))
    }
    .child(label.into())
}

/// 胶囊按钮（分类切换、加载更多等）
fn pill(id: impl Into<SharedString>, label: impl Into<SharedString>, active: bool, accent: u32) -> gpui::Stateful<gpui::Div> {
    let p = div().id(id.into()).px_4().py_1p5().rounded_full().cursor_pointer().text_sm().border_1();
    if active {
        p.bg(rgb(accent)).border_color(rgb(accent)).text_color(rgb(0x111111)).font_weight(FontWeight::MEDIUM)
    } else {
        p.bg(rgb(surface())).border_color(rgb(border())).text_color(rgb(text())).hover(|s| s.bg(rgb(hover())))
    }
    .child(label.into())
}

fn section_label(text: &'static str) -> impl IntoElement {
    div().px_3().pt_4().pb_1().text_xs().text_color(rgb(subtle())).font_weight(FontWeight::MEDIUM).child(text)
}

fn skeleton(w: f32, h: f32) -> gpui::Div {
    div().w(px(w)).h(px(h)).rounded_lg().bg(rgb(surface()))
}

impl Render for SimpleLive {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 外观设置：跟随系统时看窗口当前外观
        let light = match self.store.theme {
            Theme::Light => true,
            Theme::Dark => false,
            Theme::System => matches!(window.appearance(), gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight),
        };
        LIGHT.store(light, Ordering::Relaxed);
        if std::mem::take(&mut self.pending_fullscreen) && !window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        let content_w = f32::from(window.viewport_size().width) - SIDEBAR_W;
        // 直播间里全屏时只留视频
        let full = self.room.is_some() && window.is_fullscreen();
        let main = match &self.room {
            Some(room) => self.render_room(room, full, cx),
            None => self.render_browse(content_w, cx).into_any_element(),
        };
        let accent = self.accent();
        div()
            .key_context("SimpleLive")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev, window, cx| this.on_key(ev, window, cx)))
            .size_full()
            .flex()
            .bg(rgb(bg()))
            .text_color(rgb(text()))
            .children((!full).then(|| self.render_sidebar(cx)))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(main)
                    // 顶部细进度条代替“加载中”文字
                    .children(self.loading.then(|| div().absolute().top_0().left_0().right_0().h(px(2.)).bg(linear_gradient(90., linear_color_stop(tint(accent, 0x00), 0.), linear_color_stop(rgb(accent), 1.)))))
                    .children(self.error.clone().map(|e| {
                        div()
                            .id("error")
                            .absolute()
                            .bottom(px(20.))
                            .right(px(20.))
                            .max_w(px(420.))
                            .px_4()
                            .py_3()
                            .rounded_lg()
                            .bg(rgb(0x3a1d22))
                            .border_1()
                            .border_color(rgb(0x6b2a33))
                            .text_sm()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.error = None;
                                cx.notify()
                            }))
                            .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(0xff8a95)).child("出错了"))
                            .child(div().text_color(rgb(text())).child(e))
                            .child(div().pt_1().text_xs().text_color(rgb(muted())).child("点击关闭"))
                    })),
            )
    }
}

impl SimpleLive {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.accent();
        let in_room = self.room.is_some();
        let sites = self.sites.iter().enumerate().map(|(i, s)| {
            let a = ACCENTS[i % ACCENTS.len()];
            let active = i == self.site_idx;
            let item = div().id(SharedString::from(format!("site{i}"))).px_3().py_2().rounded_lg().cursor_pointer().flex().items_center().gap_2p5().text_sm();
            let item = if active { item.bg(rgb(hover())).text_color(rgb(text())).font_weight(FontWeight::MEDIUM) } else { item.text_color(rgb(muted())).hover(|s| s.bg(rgb(hover())).text_color(rgb(text()))) };
            item.child(div().size(px(8.)).rounded_full().bg(rgb(a))).child(s.name()).on_click(cx.listener(move |this, _, _, cx| this.switch_site(i, cx)))
        });
        // 主页设置里的页面顺序
        let tabs = self.store.home_sort.iter().filter_map(|id| Some((Tab::from_id(id)?, HOME_PAGES.iter().find(|(p, _)| p == id)?.1))).map(|(t, name)| {
            nav_item(name, name, self.tab == t && !in_room, accent).on_click(cx.listener(move |this, _, _, cx| this.go(t, cx)))
        });
        let mine = [(Tab::History, "观看记录"), (Tab::Settings, "设置")].map(|(t, name)| {
            nav_item(name, name, self.tab == t && !in_room, accent).on_click(cx.listener(move |this, _, _, cx| this.go(t, cx)))
        });
        div()
            .w(px(SIDEBAR_W))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(rgb(sidebar()))
            .border_r_1()
            .border_color(rgb(border()))
            // 透明标题栏：给红绿灯按钮留出位置
            .pt(px(48.))
            .px_3()
            .child(
                div()
                    .px_3()
                    .pb_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(26.))
                            .rounded_lg()
                            .bg(linear_gradient(135., linear_color_stop(rgb(accent), 0.), linear_color_stop(rgb(0x7c5cff), 1.)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .child("S"),
                    )
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Simple Live")),
            )
            .child(section_label("平台"))
            .child(div().flex().flex_col().gap_0p5().children(sites))
            .child(section_label("浏览"))
            .child(div().flex().flex_col().gap_0p5().children(tabs))
            .child(section_label("我的"))
            .child(div().flex().flex_col().gap_0p5().children(mine))
            .child(div().flex_1())
            .child(div().px_3().pb_4().text_xs().text_color(rgb(subtle())).child(format!("{} 暂停 · {} 全屏 · Esc 返回", settings::key_label(&self.store.key_for("pause")), settings::key_label(&self.store.key_for("fullscreen")))))
    }

    fn render_header(&self, title: String, subtitle: String, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.accent();
        let searching = self.tab == Tab::Search || !self.keyword.is_empty();
        let search_text = if self.keyword.is_empty() {
            div().text_color(rgb(subtle())).child("直接输入关键词搜索…")
        } else {
            // 输入法组字中的部分加下划线
            let underline = HighlightStyle { underline: Some(UnderlineStyle { thickness: px(1.), color: Some(rgb(accent).into()), wavy: false }), ..Default::default() };
            let styled = StyledText::new(format!("{}▏", self.keyword)).with_highlights(self.marked.clone().map(|r| (r, underline)));
            div().text_color(rgb(text())).child(styled)
        };
        let (entity, focus) = (cx.entity(), self.focus.clone());
        let mut left = div().flex().flex_col().gap_0p5().min_w(px(0.));
        if self.tab == Tab::Category && self.sub.is_some() {
            left = left.child(
                div()
                    .id("back-cats")
                    .text_xs()
                    .text_color(rgb(muted()))
                    .cursor_pointer()
                    .hover(|s| s.text_color(rgb(accent)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sub = None;
                        cx.notify()
                    }))
                    .child("‹ 全部分类"),
            );
        }
        div()
            .min_h(px(76.))
            .pb_2()
            .flex_none()
            .px(px(PAD))
            .pt(px(12.))
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(left.child(div().text_xl().font_weight(FontWeight::SEMIBOLD).truncate().child(title)).child(div().text_xs().text_color(rgb(muted())).child(subtitle)))
            .child(
                div()
                    .id("search")
                    .w(px(300.))
                    .h(px(38.))
                    .flex_none()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_full()
                    .bg(rgb(surface()))
                    .border_1()
                    .border_color(if searching { tint(accent, 0x99) } else { rgb(border()).into() })
                    .text_sm()
                    .cursor_text()
                    .on_click(cx.listener(|this, _, _, cx| this.go(Tab::Search, cx)))
                    .relative()
                    // 把文字输入（含输入法组字）接到这个搜索框
                    .child(canvas(|_, _, _| {}, move |bounds, _, window, cx| window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx)).absolute().size_full())
                    .child(div().text_color(rgb(if searching { accent } else { subtle() })).child("⌕"))
                    .child(search_text.flex_1().truncate())
                    .children((!self.keyword.is_empty()).then(|| div().text_xs().text_color(rgb(subtle())).child("⏎"))),
            )
    }

    fn render_browse(&self, content_w: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let site = self.sites[self.site_idx].name();
        let (title, subtitle) = match (self.tab, &self.sub) {
            (Tab::Recommend, _) => ("热门推荐".to_string(), format!("{site} · 正在直播")),
            (Tab::Category, None) => ("全部分类".to_string(), format!("{site} · {} 个大类", self.cats.len())),
            (Tab::Category, Some(sub)) => (sub.name.clone(), format!("{site} · 分类直播")),
            (Tab::Search, _) if self.keyword.is_empty() => ("搜索".to_string(), format!("在{site}里找直播间")),
            (Tab::Search, _) => (format!("“{}”", self.keyword), format!("{site} · 搜索结果")),
            (Tab::History, _) => ("观看记录".to_string(), format!("最近看过的 {} 个直播间", self.store.history.len())),
            (Tab::Settings, _) => ("设置".to_string(), "外观、播放、弹幕、关注与账号".to_string()),
            (Tab::Follow, _) => {
                let live = self.follow_list.iter().filter(|e| e.live == Some(true)).count();
                ("我的关注".to_string(), format!("{} 个主播 · {live} 个正在直播", self.follow_list.len()))
            }
        };
        let body = match self.tab {
            Tab::Category if self.sub.is_none() => self.render_categories(cx).into_any_element(),
            Tab::Follow => self.render_follows(content_w - 2. * PAD, cx).into_any_element(),
            Tab::History => self.render_history(content_w - 2. * PAD, cx).into_any_element(),
            Tab::Settings => self.render_settings(cx).into_any_element(),
            _ => self.render_grid(content_w - 2. * PAD, cx).into_any_element(),
        };
        image_cache(self.images.clone())
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_header(title, subtitle, cx))
            .child(div().id("browse").flex_1().overflow_y_scroll().px(px(PAD)).pb(px(PAD)).child(body))
    }

    fn render_grid(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.accent();
        // 按可用宽度算列数，卡片等宽铺满
        let cols = ((width + GAP) / (MIN_CARD + GAP)).floor().max(1.);
        let card_w = ((width - GAP * (cols - 1.)) / cols - 0.5).floor();
        let cover_h = card_w * 9. / 16.;
        let grid = div().flex().flex_wrap().gap(px(GAP));
        if self.rooms.is_empty() {
            return if self.loading {
                grid.children((0..cols as usize * 2).map(|_| div().flex().flex_col().gap_2().child(skeleton(card_w, cover_h)).child(skeleton(card_w * 0.8, 14.)).child(skeleton(card_w * 0.4, 12.)))).into_any_element()
            } else {
                let tip = if self.tab == Tab::Search && self.keyword.is_empty() { "在任意位置直接输入关键词，按回车搜索" } else { "这里还没有直播间" };
                div().pt(px(120.)).flex().justify_center().text_color(rgb(muted())).child(tip).into_any_element()
            };
        }
        let fallback = self.sites[self.site_idx].name();
        let cards = self.rooms.iter().map(|r| {
            let id = r.room_id.clone();
            self.card(SharedString::from(format!("room-{}", r.room_id)), card_w, &r.cover, &r.title, &r.user_name, Some(fmt_count(r.online)), fallback)
                .on_click(cx.listener(move |this, _, _, cx| this.open_room(id.clone(), cx)))
        });
        let more = if self.has_more {
            Some(div().pt(px(28.)).flex().justify_center().child(if self.loading {
                div().text_sm().text_color(rgb(muted())).child("加载中…").into_any_element()
            } else {
                pill("more", "加载更多", false, accent).on_click(cx.listener(|this, _, _, cx| this.load_more(cx))).into_any_element()
            }))
        } else {
            None
        };
        div().flex().flex_col().child(grid.children(cards)).children(more).into_any_element()
    }

    /// 直播间卡片：16:9 封面 + 两行标题 + 主播名；`online` 为 None 时显示“未开播”并置灰
    #[allow(clippy::too_many_arguments)]
    fn card(&self, id: SharedString, w: f32, cover: &str, title: &str, user: &str, online: Option<String>, fallback: &'static str) -> gpui::Stateful<gpui::Div> {
        let accent = self.accent();
        let live = online.is_some();
        let badge = div().absolute().bottom(px(7.)).left(px(9.)).flex().items_center().gap_1p5().text_xs().text_color(rgb(0xffffff));
        let badge = match online {
            // 空字符串表示不显示角标（观看记录）
            Some(n) if n.is_empty() => badge,
            Some(n) => badge.child(div().size(px(6.)).rounded_full().bg(rgb(0xff4757))).child(n),
            None => badge.child(div().px_1p5().rounded_sm().bg(tint(0x000000, 0x99)).child("未开播")),
        };
        div()
            .id(id)
            .group("card")
            .w(px(w))
            .flex()
            .flex_col()
            .gap_2()
            .cursor_pointer()
            .opacity(if live { 1. } else { 0.55 })
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(w * 9. / 16.))
                    .rounded_lg()
                    .overflow_hidden()
                    .bg(rgb(surface()))
                    .border_2()
                    .border_color(tint(accent, 0x00))
                    .group_hover("card", |s| s.border_color(rgb(accent)))
                    .child(
                        img(SharedString::from(cover.to_string()))
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .with_loading(|| div().size_full().bg(rgb(surface())).into_any_element())
                            .with_fallback(move || div().size_full().flex().items_center().justify_center().text_color(rgb(subtle())).child(fallback).into_any_element()),
                    )
                    .child(div().absolute().bottom_0().left_0().right_0().h(px(44.)).bg(linear_gradient(180., linear_color_stop(tint(0x000000, 0x00), 0.), linear_color_stop(tint(0x000000, 0xb0), 1.))))
                    .child(badge),
            )
            .child(div().h(px(40.)).text_sm().line_height(px(20.)).font_weight(FontWeight::MEDIUM).line_clamp(2).group_hover("card", |s| s.text_color(rgb(accent))).child(title.to_string()))
            .child(div().text_xs().text_color(rgb(muted())).truncate().child(user.to_string()))
    }

    fn render_follows(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        if self.follow_list.is_empty() {
            return div().pt(px(120.)).flex().justify_center().text_color(rgb(muted())).child("还没有关注的主播，进入直播间点「关注」").into_any_element();
        }
        let cols = ((width + GAP) / (MIN_CARD + GAP)).floor().max(1.);
        let card_w = ((width - GAP * (cols - 1.)) / cols - 0.5).floor();
        let cards = self.follow_list.iter().enumerate().map(|(i, e)| {
            let f = &e.follow;
            let site = self.site_index(&f.site).map_or("", |s| self.sites[s].name());
            let online = match e.live {
                Some(true) => Some(fmt_count(e.online)),
                // 还没查到时先当在播，不急着置灰
                None => Some("查询中".to_string()),
                Some(false) => None,
            };
            self.card(SharedString::from(format!("follow-{}-{}", f.site, f.room_id)), card_w, &f.cover, &f.title, &format!("{} · {site}", f.user_name), online, site)
                .on_click(cx.listener(move |this, _, _, cx| this.open_follow(i, cx)))
        });
        div().flex().flex_wrap().gap(px(GAP)).children(cards).into_any_element()
    }

    /// 横向滑块：点击或拖动时把比例交给 `set`
    fn slider(&self, idx: usize, frac: f32, width: f32, set: fn(&mut Self, f32, &mut Context<Self>), cx: &mut Context<Self>) -> impl IntoElement {
        let (bar, accent) = (self.sliders[idx].clone(), self.accent());
        let frac = frac.clamp(0., 1.);
        div()
            .id(SharedString::from(format!("slider-{idx}")))
            .relative()
            .w(px(width))
            .h(px(24.))
            .flex()
            .items_center()
            .cursor_pointer()
            .child(canvas(move |b, _, _| bar.set(b), |_, _, _, _| {}).absolute().size_full())
            // 中性灰轨道 + 带细边的白色拖柄：深色播放器和浅色设置页上都看得清
            .child(div().w_full().h(px(4.)).rounded_full().bg(tint(0x8a93a3, 0x66)).child(div().h_full().w(relative(frac)).rounded_full().bg(rgb(accent))))
            .child(div().absolute().left(relative(frac)).ml(px(-6.)).size(px(12.)).rounded_full().bg(rgb(0xffffff)).border_1().border_color(tint(0x000000, 0x33)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                let f = this.slider_frac(idx, ev.position.x);
                set(this, f, cx)
            }))
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                if ev.pressed_button == Some(MouseButton::Left) {
                    let f = this.slider_frac(idx, ev.position.x);
                    set(this, f, cx)
                }
            }))
    }

    /// 弹幕设置面板：字号、不透明度、滚动速度、显示区域（范围同原项目，字号收窄到 12-60）
    fn render_danmaku_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let st = self.store.danmaku;
        let row = |label: &'static str, value: String, slider: gpui::AnyElement| {
            div().flex().flex_col().gap_1().px_3().py_2().child(div().flex().justify_between().text_sm().child(label).child(div().text_color(rgb(muted())).child(value))).child(slider)
        };
        // 回调要是不捕获变量的 fn，直接在闭包里按比例换算并取整
        div()
            .flex()
            .flex_col()
            .child(row("字体大小", format!("{}", st.font), self.slider(1, (st.font - 12.) / 48., 192., |t, f, cx| t.set_danmaku_style(DanmakuStyle { font: (12. + f * 48.).round(), ..t.store.danmaku }, cx), cx).into_any_element()))
            .child(row("不透明度", format!("{}%", (st.opacity * 100.).round()), self.slider(2, (st.opacity - 0.1) / 0.9, 192., |t, f, cx| t.set_danmaku_style(DanmakuStyle { opacity: ((0.1 + f * 0.9) * 20.).round() / 20., ..t.store.danmaku }, cx), cx).into_any_element()))
            .child(row("滚动速度", format!("{} 秒", st.secs), self.slider(3, (st.secs - 1.) / 29., 192., |t, f, cx| t.set_danmaku_style(DanmakuStyle { secs: (1. + f * 29.).round(), ..t.store.danmaku }, cx), cx).into_any_element()))
            .child(row("显示区域", format!("{}%", (st.area * 100.).round()), self.slider(4, (st.area - 0.1) / 0.9, 192., |t, f, cx| t.set_danmaku_style(DanmakuStyle { area: ((0.1 + f * 0.9) * 20.).round() / 20., ..t.store.danmaku }, cx), cx).into_any_element()))
            .child(div().px_3().pt_1().text_xs().text_color(rgb(subtle())).child("滚动速度为弹幕横穿画面的秒数，越小越快"))
            .into_any_element()
    }

    fn render_categories(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.accent();
        if self.cats.is_empty() {
            return div().flex().flex_wrap().gap_3().children((0..24).map(|_| skeleton(120., 104.))).into_any_element();
        }
        let tabs = self.cats.iter().enumerate().map(|(i, c)| {
            pill(SharedString::from(format!("cat{i}")), c.name.clone(), i == self.cat_idx, accent).on_click(cx.listener(move |this, _, _, cx| {
                this.cat_idx = i;
                cx.notify()
            }))
        });
        let subs = self.cats.get(self.cat_idx).map(|c| c.children.as_slice()).unwrap_or_default().iter().enumerate().map(|(i, s)| {
            let sub = s.clone();
            div()
                .id(SharedString::from(format!("sub{i}")))
                .w(px(120.))
                .p_3()
                .rounded_xl()
                .bg(rgb(surface()))
                .border_1()
                .border_color(rgb(border()))
                .cursor_pointer()
                .hover(|st| st.bg(rgb(hover())).border_color(tint(accent, 0x88)))
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .on_click(cx.listener(move |this, _, _, cx| this.open_sub(sub.clone(), cx)))
                .child(
                    img(SharedString::from(s.pic.clone()))
                        .size(px(56.))
                        .rounded_lg()
                        .object_fit(ObjectFit::Cover)
                        .with_fallback(|| div().size(px(56.)).rounded_lg().bg(rgb(hover())).into_any_element()),
                )
                .child(div().w_full().text_xs().text_center().truncate().child(s.name.clone()))
        });
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(div().flex().flex_wrap().gap_2().children(tabs))
            .child(div().flex().flex_wrap().gap_3().children(subs))
            .into_any_element()
    }

    /// 底部控制栏，布局照原项目：左侧 刷新/播放/弹幕，右侧 音量/清晰度/线路/全屏
    fn render_controls(&self, room: &Room, full: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.accent();
        let playing = room.player.is_some();
        let white = rgb(0xffffff);
        let icon_btn = |id: &'static str, name: &'static str| {
            div().id(id).size(px(36.)).flex_none().rounded_full().flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(tint(0xffffff, 0x26))).child(icon(name, 22., white))
        };
        let text_btn = |id: &'static str, label: String, open: bool| {
            let b = div().id(id).h(px(32.)).px_3().rounded_md().flex().items_center().cursor_pointer().text_sm().text_color(white).hover(|s| s.bg(tint(0xffffff, 0x26)));
            if open { b.bg(tint(accent, 0x55)) } else { b }.child(label)
        };
        let level = if self.muted { 0. } else { self.volume as f32 / 100. };
        let volume_slider = self.slider(0, level, 88., |t, f, cx| t.set_volume((f * 100.).round() as u8, false, cx), cx);
        let quality = room.qualities.get(room.quality_idx).map_or("清晰度".to_string(), |q| q.name.clone());
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(64.))
            .px_3()
            .pt_3()
            .flex()
            .items_center()
            .gap_1()
            .bg(linear_gradient(180., linear_color_stop(tint(0x000000, 0x00), 0.), linear_color_stop(tint(0x000000, 0xc0), 1.)))
            // 播放中悬停才显示；暂停或面板打开时常驻
            .opacity(if playing && self.menu.is_none() { 0. } else { 1. })
            .group_hover("video", |s| s.opacity(1.))
            // 控件上的点击不要冒泡成视频的双击全屏
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(icon_btn("refresh", "refresh").on_click(cx.listener(|this, _, _, cx| {
                let idx = this.room.as_ref().map_or(0, |r| r.quality_idx);
                this.choose_quality(idx, cx)
            })))
            .child(icon_btn("play", if playing { "pause" } else { "play" }).on_click(cx.listener(|this, _, _, cx| this.play(cx))))
            .child(icon_btn("danmaku", if self.show_danmaku { "danmaku_on" } else { "danmaku_off" }).on_click(cx.listener(|this, _, _, cx| this.toggle_danmaku(cx))))
            .child(icon_btn("danmaku-settings", "tune").on_click(cx.listener(|this, _, _, cx| {
                this.menu = if this.menu == Some(Menu::Danmaku) { None } else { Some(Menu::Danmaku) };
                cx.notify()
            })))
            .child(div().flex_1())
            .child(icon_btn("mute", if self.muted || self.volume == 0 { "volume_off" } else { "volume" }).on_click(cx.listener(|this, _, _, cx| {
                let v = if this.volume == 0 { 50 } else { this.volume };
                this.set_volume(v, !this.muted && this.volume > 0, cx)
            })))
            .child(volume_slider)
            .child(div().w(px(8.)))
            .child(text_btn("quality", quality, self.menu == Some(Menu::Quality)).on_click(cx.listener(|this, _, _, cx| {
                this.menu = if this.menu == Some(Menu::Quality) { None } else { Some(Menu::Quality) };
                cx.notify()
            })))
            .child(text_btn("line", format!("线路{}", room.line_idx + 1), self.menu == Some(Menu::Line)).on_click(cx.listener(|this, _, _, cx| {
                this.menu = if this.menu == Some(Menu::Line) { None } else { Some(Menu::Line) };
                cx.notify()
            })))
            .child(icon_btn("fullscreen", if full { "fullscreen_exit" } else { "fullscreen" }).on_click(|_, window, _| window.toggle_fullscreen()))
    }

    /// 右侧选择面板（同原项目的 showRightDialog）：清晰度或线路
    fn render_menu(&self, room: &Room, menu: Menu, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.accent();
        let (title, items, selected): (&str, Vec<(String, Option<&str>)>, usize) = match menu {
            Menu::Quality => ("清晰度", room.qualities.iter().map(|q| (q.name.clone(), None)).collect(), room.quality_idx),
            Menu::Line => ("线路选择", room.urls.iter().enumerate().map(|(i, u)| (format!("线路{}", i + 1), Some(if u.contains(".flv") { "FLV" } else { "HLS" }))).collect(), room.line_idx),
            Menu::Danmaku => ("弹幕设置", vec![], 0),
        };
        let rows = items.into_iter().enumerate().map(|(i, (label, badge))| {
            let on = i == selected;
            div()
                .id(SharedString::from(format!("menu-{i}")))
                .h(px(40.))
                .px_3()
                .rounded_lg()
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .text_sm()
                .text_color(if on { rgb(accent) } else { rgb(text()) })
                .hover(|s| s.bg(tint(0xffffff, 0x14)))
                .on_click(cx.listener(move |this, _, _, cx| match menu {
                    Menu::Quality => this.choose_quality(i, cx),
                    Menu::Line => this.choose_line(i, cx),
                    Menu::Danmaku => {}
                }))
                .child(div().flex_1().child(label))
                .children(badge.map(|b| div().px_1p5().rounded_sm().border_1().border_color(tint(0xffffff, 0x40)).text_xs().text_color(rgb(muted())).child(b)))
                .children(on.then(|| icon("check", 18., rgb(accent))))
        });
        div()
            .id("menu")
            .absolute()
            .top_0()
            .right_0()
            // 停在控制栏上方，面板打开时清晰度/线路按钮仍可点
            .bottom(px(64.))
            .w(px(240.))
            .p_3()
            .flex()
            .flex_col()
            .gap_1()
            .bg(rgb(0x111318))
            .border_l_1()
            .border_color(tint(0xffffff, 0x14))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().px_3().pt_2().pb_3().text_base().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(if menu == Menu::Danmaku {
                div().id("menu-list").flex_1().child(self.render_danmaku_settings(cx))
            } else {
                div().id("menu-list").flex_1().overflow_y_scroll().flex().flex_col().gap_0p5().children(rows)
            })
    }

    fn render_room(&self, room: &Room, full: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let accent = self.accent();
        let d = &room.detail;
        let chip = |text: String| div().px_2p5().py_1().rounded_md().bg(rgb(surface())).border_1().border_color(rgb(border())).text_xs().text_color(rgb(muted())).child(text);
        let header = div()
            .flex_none()
            .px(px(20.))
            .pt(px(40.))
            .pb(px(14.))
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id("back")
                    .size(px(34.))
                    .flex_none()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(rgb(surface()))
                    .hover(|s| s.bg(rgb(hover())))
                    .text_lg()
                    .on_click(cx.listener(|this, _, _, cx| this.close_room(cx)))
                    .child("‹"),
            )
            .child(
                img(SharedString::from(d.user_avatar.clone()))
                    .size(px(40.))
                    .flex_none()
                    .rounded_full()
                    .object_fit(ObjectFit::Cover)
                    .with_fallback(move || div().size(px(40.)).rounded_full().bg(rgb(accent)).into_any_element()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).truncate().child(d.title.clone()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_color(rgb(muted()))
                            .child(d.user_name.clone())
                            .child(if d.live {
                                div().px_1p5().rounded_sm().bg(rgb(0xff4757)).text_color(rgb(0xffffff)).font_weight(FontWeight::MEDIUM).child("LIVE")
                            } else {
                                div().px_1p5().rounded_sm().bg(rgb(hover())).child("未开播")
                            })
                            .child(format!("{} 人气", fmt_count(room.online))),
                    ),
            )
            .child(div().px_2p5().py_1().rounded_md().bg(tint(accent, 0x22)).text_xs().text_color(rgb(accent)).font_weight(FontWeight::MEDIUM).child(room.qualities.get(room.quality_idx).map_or(String::new(), |q| q.name.clone())))
            .child(chip(format!("{} 条线路", room.urls.len())))
            .child({
                let followed = self.store.is_followed(self.site().id(), &d.room_id);
                let b = div().id("follow").h(px(30.)).px_3().rounded_full().flex().items_center().cursor_pointer().text_xs().font_weight(FontWeight::MEDIUM);
                if followed { b.bg(rgb(surface())).border_1().border_color(rgb(border())).text_color(rgb(muted())).hover(|s| s.bg(rgb(hover()))).child("已关注") } else { b.bg(rgb(accent)).text_color(rgb(0x111111)).child("＋ 关注") }
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_follow(cx)))
            });

        let controls = self.render_controls(room, full, cx);
        let video = div()
            .id("video")
            .group("video")
            .relative()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .rounded_xl()
            .overflow_hidden()
            .border_1()
            .border_color(rgb(border()))
            .on_click(|ev, window, _| {
                if ev.click_count() >= 2 {
                    window.toggle_fullscreen()
                }
            })
            .child(self.video.clone())
            .child(controls)
            .children(self.menu.map(|m| self.render_menu(room, m, cx)));
        if full {
            return div().size_full().child(video.rounded_none().border_0()).into_any_element();
        }

        let mut chat_box = StyleRefinement::default();
        chat_box = chat_box.w(px(CHAT_W)).h_full().flex_none();
        let chat = AnyView::from(room.chat.clone()).cached(chat_box);

        div().size_full().flex().flex_col().child(header).child(div().flex_1().min_h(px(0.)).px(px(20.)).pb(px(20.)).flex().gap_4().child(video).children(self.show_chat.then_some(chat))).into_any_element()
    }
}

/// 输入法接口用 UTF-16 下标，关键词按 UTF-8 存，这里互相换算
fn utf16_to_byte(s: &str, i: usize) -> usize {
    let mut n = 0;
    for (b, c) in s.char_indices() {
        if n >= i {
            return b;
        }
        n += c.len_utf16();
    }
    s.len()
}

fn utf16_to_bytes(s: &str, r: Range<usize>) -> Range<usize> {
    utf16_to_byte(s, r.start)..utf16_to_byte(s, r.end)
}

fn bytes_to_utf16(s: &str, r: Range<usize>) -> Range<usize> {
    s[..r.start].encode_utf16().count()..s[..r.end].encode_utf16().count()
}

/// 搜索框的文字输入：光标始终在末尾，支持输入法组字（marked text）
impl EntityInputHandler for SimpleLive {
    fn text_for_range(&mut self, range: Range<usize>, adjusted: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let r = utf16_to_bytes(&self.keyword, range);
        *adjusted = Some(bytes_to_utf16(&self.keyword, r.clone()));
        Some(self.keyword[r].to_string())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let end = self.keyword.encode_utf16().count();
        Some(UTF16Selection { range: end..end, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone().map(|r| bytes_to_utf16(&self.keyword, r))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        let r = self.input_range(range);
        self.keyword.replace_range(r, text);
        self.marked = None;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, _: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        let r = self.input_range(range);
        self.keyword.replace_range(r.clone(), text);
        self.marked = (!text.is_empty()).then(|| r.start..r.start + text.len());
        cx.notify();
    }

    fn bounds_for_range(&mut self, range: Range<usize>, element: gpui::Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<gpui::Bounds<Pixels>> {
        // 候选框跟着光标走：文字起点约在左内边距+图标之后，按字符估算横向位置
        let r = utf16_to_bytes(&self.keyword, range);
        let x = 38. + self.keyword[..r.start].chars().map(|c| if c as u32 >= 0x2E80 { 14. } else { 7.5 }).sum::<f32>();
        Some(gpui::Bounds::new(point(element.left() + px(x), element.top()), size(px(2.), element.size.height)))
    }

    fn character_index_for_point(&mut self, _: gpui::Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.keyword.encode_utf16().count())
    }
}

fn main() {
    Application::new().with_assets(icons::Icons).with_http_client(Arc::new(img_http::ImageClient::new())).run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(960.), px(600.))),
            titlebar: Some(TitlebarOptions { title: Some("Simple Live".into()), appears_transparent: true, traffic_light_position: Some(point(px(16.), px(16.))) }),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| {
                // 外观设置“跟随系统”：系统切换深浅色时重绘
                cx.observe_window_appearance(window, |this: &mut SimpleLive, _, cx| this.refresh_theme(cx)).detach();
                SimpleLive::new(cx)
            });
            window.focus(&view.read(cx).focus);
            view
        })
        .expect("open window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_compact() {
        assert_eq!(fmt_count(999), "999");
        assert_eq!(fmt_count(12_345), "1.2万");
        assert_eq!(fmt_count(320_000_000), "3.2亿");
    }

    #[test]
    fn utf16_ranges_roundtrip() {
        let s = "a中😀b";
        assert_eq!(utf16_to_bytes(s, 1..2), 1..4);
        assert_eq!(utf16_to_bytes(s, 2..4), 4..8);
        assert_eq!(utf16_to_bytes(s, 5..9), 9..9);
        assert_eq!(bytes_to_utf16(s, 4..8), 2..4);
    }

    #[test]
    fn name_colors_are_stable() {
        assert_eq!(name_color("小明"), name_color("小明"));
    }
}
