mod about;
mod icons;
mod img_http;
mod menus;
mod mpv;
mod settings;
mod store;
#[allow(dead_code)] // 设计系统导出的完整色板与尺寸，不是每个都用到
mod theme;
mod ui;
#[cfg(target_os = "macos")]
mod mpv_gl;

use gpui::{
    anchored, canvas, deferred, div, ClipboardItem, ease_out_quint, image_cache, img, list, relative, Animation, AnimationExt, AnyElement, BoxShadow, ShapedLine, ListAlignment, ListState, AnyView, ElementInputHandler, EntityInputHandler, UTF16Selection, UnderlineStyle, linear_color_stop, linear_gradient, point, prelude::*, px, rgb, size, App, Application, Bounds, Context, Entity, FocusHandle, FontWeight,
    HighlightStyle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, Pixels, PromptButton, PromptLevel, RenderImage, RetainAllImageCache, Rgba, SharedString, StyleRefinement, StyledText, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use icons::icon;
use livo_core::{danmaku, AnchorItem, Category, LiveMessage, PlayQuality, RoomDetail, RoomItem, RoomPage, Site, SubCategory};
use std::cell::Cell;
use std::ops::Range;
use livo_core::bilibili::BiliBili;
use store::{DanmakuStyle, Follow, History, Store, Theme, HOME_PAGES, KEY_ACTIONS};
use std::rc::Rc;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;
// 颜色、字号与通用控件都来自设计系统（theme.rs 是导出的 tokens，ui.rs 是控件）
use theme::{CARD_MIN, CHAT_WIDTH, RADIUS_LG, RADIUS_MD, RADIUS_SM, RADIUS_XS, ROW_HEIGHT, SIDEBAR_WIDTH, SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_6, TOOLBAR_HEIGHT};
use ui::*;

/// 内容区页边距与卡片网格间距
const PAD: f32 = SPACE_6;
const GAP: f32 = SPACE_4;
const MAX_DANMAKU: usize = 200;
/// 滚动弹幕同一弹道前后两条的间距
const DM_GAP: f32 = 24.;

/// 断流：所有线路都断开了，或者 10 秒没有新画面；还没出过画面时放宽到 30 秒，留给首次缓冲
fn is_stalled(idle: bool, since: Instant, last_frame: Option<Instant>, now: Instant) -> bool {
    idle || match last_frame {
        Some(t) => now.duration_since(t) > Duration::from_secs(10),
        None => now.duration_since(since) > Duration::from_secs(30),
    }
}

/// 时移：落后直播不到这么多秒就当作在直播（直播流自带 3-6 秒延迟）
const LIVE_EDGE: f64 = 10.;

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn rt() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    // 只跑网络请求和弹幕连接，两个线程足够；默认每个核一个线程
    RT.get_or_init(|| tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("tokio runtime"))
}

/// 12345 -> "1.2万"
fn fmt_count(n: i64) -> String {
    match n {
        n if n >= 100_000_000 => format!("{:.1}亿", n as f64 / 1e8),
        n if n >= 10_000 => format!("{:.1}万", n as f64 / 1e4),
        n => n.to_string(),
    }
}

/// 复制链接用的直播间网页地址
fn room_url(site: &str, room_id: &str) -> String {
    match site {
        "bilibili" => format!("https://live.bilibili.com/{room_id}"),
        "douyu" => format!("https://www.douyu.com/{room_id}"),
        "huya" => format!("https://www.huya.com/{room_id}"),
        _ => String::new(),
    }
}

/// 开播时长：「开播 2 小时 14 分」「开播 14 分钟」；不知道开播时间时为 None
fn fmt_live_for(start: i64, now: i64) -> Option<String> {
    let secs = now.checked_sub(start).filter(|_| start > 0)?;
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    Some(match (h, m) {
        (0, 0) => "刚开播".into(),
        (0, m) => format!("开播 {m} 分钟"),
        (h, 0) => format!("开播 {h} 小时"),
        (h, m) => format!("开播 {h} 小时 {m} 分"),
    })
}

/// 时移落后的时长：42 秒 -> "-00:42"
fn fmt_behind(secs: f64) -> String {
    let s = secs.max(0.).round() as u64;
    format!("-{:02}:{:02}", s / 60, s % 60)
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Recommend,
    Category,
    Search,
    Follow,
    History,
    About,
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
    price: Option<i64>,
    /// 系统消息（连接状态变化）：12px text-subtle，没有用户名
    system: bool,
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

/// 后退 / 前进记录的一个位置：页面（含平台筛选、子分区、搜索词）或某个直播间
#[derive(Clone)]
struct Loc {
    tab: Tab,
    all: bool,
    site: &'static str,
    sub: Option<SubCategory>,
    keyword: String,
    room: Option<(&'static str, String)>,
}

impl PartialEq for Loc {
    fn eq(&self, o: &Self) -> bool {
        (self.tab, self.all, self.site, &self.keyword, &self.room) == (o.tab, o.all, o.site, &o.keyword, &o.room) && self.sub.as_ref().map(|s| &s.id) == o.sub.as_ref().map(|s| &s.id)
    }
}

/// 菜单项被点中时要做的事
type Act = Rc<dyn Fn(&mut Livo, &mut Window, &mut Context<Livo>)>;

/// 右键菜单 / 弹出按钮的菜单：出现在鼠标位置，点外面或 Esc 关掉
struct CtxMenu {
    pos: gpui::Point<Pixels>,
    entries: Vec<MenuEntry<Act>>,
    /// 在设置窗口里弹出的菜单只画在设置窗口
    settings: bool,
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
    /// 第一次绘制时排好的（正文, 阴影），之后每帧只换位置重画，不再排版
    shaped: Option<(ShapedLine, ShapedLine)>,
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
    /// 本轮播放开始的时刻和最近一帧到达的时刻，给断流看门狗用
    playing_since: Option<Instant>,
    last_frame: Option<Instant>,
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
        self.bullets.push(Bullet { text: text.into(), color, lane, start: now, width, shaped: None });
    }

    /// 满屏弹幕时每条都做成元素会让每帧的布局和排版吃掉大半主线程，所以直接在一个 canvas 上画排好的字
    fn render_bullets(&mut self, window: &mut Window) -> impl IntoElement {
        let (w, _) = self.size.get();
        let st = self.style;
        let (now, speed) = (Instant::now(), w / st.secs);
        let x_of = |b: &Bullet| w - now.duration_since(b.start).as_secs_f32() * speed;
        self.bullets.retain(|b| x_of(b) + b.width > 0.);
        let font = gpui::Font { weight: FontWeight::SEMIBOLD, ..window.text_style().font() };
        let lines: Vec<_> = self
            .bullets
            .iter_mut()
            .map(|b| {
                if b.shaped.is_none() {
                    let shape = |color: Rgba| {
                        let run = gpui::TextRun { len: b.text.len(), font: font.clone(), color: color.into(), background_color: None, underline: None, strikethrough: None };
                        window.text_system().shape_line(b.text.clone(), px(st.font), &[run], None)
                    };
                    // 没有描边能力，用 danmaku-stroke 色的字在四个斜向各错开一点画一遍，近似描边
                    b.shaped = Some((shape(rgb(b.color)), shape(c(theme::DARK.danmaku_stroke))));
                }
                (point(px(x_of(b)), px(8. + b.lane as f32 * st.font * 1.5)), b.shaped.clone().unwrap())
            })
            .collect();
        let line_height = px(st.font * 1.3);
        div().absolute().top_0().left_0().size_full().overflow_hidden().opacity(st.opacity).child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    for (pos, (text, stroke)) in &lines {
                        let at = bounds.origin + *pos;
                        // 3px danmaku-stroke 描边：八个方向各错开 1.5px
                        for (dx, dy) in [(-1.5, 0.), (1.5, 0.), (0., -1.5), (0., 1.5), (-1.1, -1.1), (1.1, -1.1), (-1.1, 1.1), (1.1, 1.1)] {
                            stroke.paint(at + point(px(dx), px(dy)), line_height, window, cx).ok();
                        }
                        text.paint(at, line_height, window, cx).ok();
                    }
                },
            )
            .size_full(),
        )
    }
}

impl Render for Video {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
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
            (Some(_), None) => screen.t_body().text_color(c(theme::DARK.text_muted)).child("正在缓冲…"),
            (None, _) => screen.t_body().text_color(c(theme::DARK.text_muted)).child("已暂停"),
        };
        // 视频帧每到一帧就重绘一次，弹幕位置随之前进
        let show = self.overlay && self.view.is_some();
        screen.children(show.then(|| self.render_bullets(window)))
    }
}

struct Livo {
    sites: Vec<Arc<Site>>,
    site_idx: usize,
    /// 平台筛选「全部」：推荐和搜索合并三个平台；分区页总是按 site_idx 的单个平台
    all: bool,
    focus: FocusHandle,
    tab: Tab,
    /// 最近一次停留的平台页面（推荐/分类/搜索）
    site_tab: Tab,
    source: Source,
    /// 房间列表，每项带来源平台（「全部」时混排）
    rooms: Vec<(&'static str, RoomItem)>,
    /// 工具栏的后退 / 前进
    back_stack: Vec<Loc>,
    fwd_stack: Vec<Loc>,
    /// 侧栏收起（⌃⌘S）
    show_sidebar: bool,
    ctx_menu: Option<CtxMenu>,
    /// 控制条显示到这个时刻：鼠标在画面上移动就往后推 3 秒，离开画面立即到期
    controls_until: Instant,
    /// 画中画小窗（⌥⌘P）
    pip: Option<gpui::WindowHandle<Pip>>,
    /// 设置窗口（⌘,）
    settings_win: Option<gpui::WindowHandle<settings::SettingsWindow>>,
    /// 关注页排序：0 在播优先，1 最近关注
    follow_sort: usize,
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
    /// 各滑块的位置，绘制时回填，用来把鼠标坐标换算成数值：0 音量，1-4 弹幕设置，5 时移
    sliders: [Rc<Cell<gpui::Bounds<Pixels>>>; 6],
    /// 全屏时控制栏滑块的放大倍数（随窗口宽度），窗口模式为 1
    ui_scale: f32,
    update: about::Update,
    /// 播放器的时移范围（见 Player::timeshift），每秒读一次给控制栏用
    timeshift: Option<(f64, f64, f64)>,
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
    /// 断流看门狗连续重试的次数，出画面后清零
    stall_retries: u8,
    /// 搜索结果里的主播（第一页），每项带来源平台
    anchors: Vec<(&'static str, AnchorItem)>,
    /// 搜索结果的主播默认只列 3 位，展开后全部显示
    anchors_all: bool,
    /// 播放器调试信息面板：是否显示、最近一次读到的内容、刷新循环的代号（关掉或重开时让旧循环退出）
    show_stats: bool,
    stats: Vec<(&'static str, String)>,
    stats_gen: usize,
    /// ⌘F 之后搜索框显示为聚焦状态，Esc 或离开页面时取消
    search_active: bool,
    /// 窗口底部的 Toast（短操作的结果），2 秒后消失；代号让旧的定时器失效
    toast: Option<SharedString>,
    toast_gen: usize,
    /// 页面代号：切换页面时加一，新页面内容按它播一次淡入
    page_gen: usize,
    /// 刚被点击的控件和代号：只有它播放过渡动画，其余控件直接画最终状态，打开页面时不会集体动一遍
    anim: (&'static str, usize),
    /// 浏览页的封面/分类图。GPUI 默认的图片缓存只增不减（内存 + 显存各一份），
    /// 所以浏览页用自己的缓存，换页面或进直播间时整个换掉，旧图随之释放
    images: Entity<RetainAllImageCache>,
}

impl Livo {
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
            all: true,
            focus: cx.focus_handle(),
            tab: Tab::Recommend,
            site_tab: Tab::Recommend,
            source: Source::Recommend,
            rooms: vec![],
            back_stack: vec![],
            fwd_stack: vec![],
            show_sidebar: true,
            ctx_menu: None,
            pip: None,
            settings_win: None,
            controls_until: Instant::now(),
            follow_sort: 0,
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
            ui_scale: 1.,
            update: Default::default(),
            timeshift: None,
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
            stall_retries: 0,
            anchors: vec![],
            anchors_all: false,
            show_stats: false,
            stats: vec![],
            stats_gen: 0,
            search_active: false,
            toast: None,
            toast_gen: 0,
            page_gen: 0,
            anim: ("", 0),
        };
        // 主页设置里排第一的页面是启动页
        let first = s.store.home_sort.iter().find_map(|id| Tab::from_id(id)).unwrap_or(Tab::Recommend);
        s.site_tab = s.store.home_sort.iter().filter_map(|id| Tab::from_id(id)).find(|t| t.is_site_page()).unwrap_or(Tab::Recommend);
        s.go(first, cx);
        // 侧栏「关注」要一启动就显示「N 在播」，不等进关注页
        if first != Tab::Follow && !s.store.follows.is_empty() {
            s.load_follows(cx);
        }
        #[cfg(debug_assertions)]
        s.debug_start(cx);
        if s.store.auto_update {
            s.check_update(true, cx);
        }
        // 开着不关的也能收到更新
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(6 * 3600)).await;
            let alive = this.update(cx, |this, cx| {
                if this.store.auto_update {
                    this.check_update(true, cx);
                }
            });
            if alive.is_err() {
                break;
            }
        })
        .detach();
        // 已下载好的新版本在退出时安装
        cx.on_app_quit(|this, cx| {
            // 记下窗口位置（render 里随时更新）
            this.store.save();
            this.install_update(false, cx);
            async {}
        })
        .detach();
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
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            if this.update(cx, |this, cx| this.check_stall(cx)).is_err() {
                break;
            }
        })
        .detach();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let alive = this.update(cx, |this, cx| {
                let ts = this.player().and_then(|p| p.timeshift());
                if ts != this.timeshift {
                    this.timeshift = ts;
                    cx.notify();
                }
            });
            if alive.is_err() {
                break;
            }
        })
        .detach();
        s
    }

    /// 断流看门狗（参照原项目）：断流（见 is_stalled）就重新取地址再播，斗鱼/虎牙的地址几分钟就过期，断开后只能重新签名。
    /// 连续 3 次没恢复就暂停并提示，免得没完没了地重试
    fn check_stall(&mut self, cx: &mut Context<Self>) {
        // 正在取新地址时不重复触发
        if self.loading {
            return;
        }
        let Some(player) = self.player() else { return };
        // 暂停时本来就没有新画面，不算断流
        if player.paused() {
            return;
        }
        let v = self.video.read(cx);
        let (Some(since), last) = (v.playing_since, v.last_frame) else { return };
        if !is_stalled(player.idle(), since, last, Instant::now()) {
            if last.is_some() {
                self.stall_retries = 0;
            }
            return;
        }
        if self.stall_retries >= 3 {
            self.error = Some("直播流中断，自动重试 3 次仍未恢复，稍后可以点刷新再试".into());
            return self.play(cx); // 播放中调用 play 是暂停
        }
        self.stall_retries += 1;
        // 旧播放器先留着：网络没恢复、取地址失败时它仍处于断流状态，下一轮检查会接着重试
        let idx = self.room.as_ref().map_or(0, |r| r.quality_idx);
        self.choose_quality(idx, cx);
    }

    /// 记下刚操作的控件，让它播放一次过渡动画
    fn touch(&mut self, key: &'static str) {
        self.anim = (key, self.anim.1 + 1);
    }

    /// 该控件正在过渡时返回动画代号
    fn anim_gen(&self, key: &str) -> Option<usize> {
        (self.anim.0 == key).then_some(self.anim.1)
    }

    fn site(&self) -> Arc<Site> {
        self.sites[self.site_idx].clone()
    }

    /// 换平台筛选：None 为「全部」。分区页只能选单个平台；关注、观看记录、关于不分平台，回到上次浏览的平台页面
    fn set_filter(&mut self, site: Option<usize>, cx: &mut Context<Self>) {
        if let Some(i) = site.filter(|&i| i != self.site_idx) {
            self.site_idx = i;
            self.cats.clear();
            self.cat_idx = 0;
            self.sub = None;
        }
        self.all = site.is_none() && self.tab != Tab::Category;
        let tab = if self.tab.is_site_page() { self.tab } else { self.site_tab };
        self.go(tab, cx);
    }

    /// 调试构建专用：LIVO_START 直接打开某个页面，给截图核对界面用（不用模拟点击）。
    /// 取值：category、follow、history、about、settings[:标签序号]、search:关键词、room:平台:房间号、site:序号（单个平台的推荐）、nosidebar
    #[cfg(debug_assertions)]
    fn debug_start(&mut self, cx: &mut Context<Self>) {
        let Ok(start) = std::env::var("LIVO_START") else { return };
        let mut parts = start.splitn(3, ':');
        match (parts.next().unwrap_or(""), parts.next(), parts.next()) {
            ("category", ..) => self.nav(cx, |t, cx| t.go(Tab::Category, cx)),
            ("follow", ..) => self.nav(cx, |t, cx| t.go(Tab::Follow, cx)),
            ("history", ..) => self.nav(cx, |t, cx| t.go(Tab::History, cx)),
            ("about", ..) => self.nav(cx, |t, cx| t.go(Tab::About, cx)),
            ("nosidebar", ..) => self.show_sidebar = false,
            // 在窗口中间弹出一个直播间右键菜单（核对菜单样式；没法模拟右键时用）
            ("menu", ..) => {
                let f = Follow { site: "douyu".into(), room_id: "252140".into(), user_name: "示例".into(), ..Default::default() };
                let remove = MenuEntry::item("从历史中移除", Some("trash-2"), Rc::new(|_: &mut Livo, _: &mut Window, _: &mut Context<Livo>| {}) as Act).danger();
                let m = self.room_menu(f, true, vec![remove]);
                self.open_ctx(point(px(520.), px(260.)), m, false, cx)
            }
            ("site", Some(i), _) => {
                let i = i.parse().unwrap_or(0);
                self.nav(cx, |t, cx| t.set_filter(Some(i), cx))
            }
            ("search", Some(kw), _) => {
                self.keyword = kw.to_string();
                self.nav(cx, |t, cx| t.go(Tab::Search, cx))
            }
            ("room", Some(site), Some(id)) => {
                let (site, id) = (self.site_index(site).map_or("", |i| self.sites[i].id()), id.to_string());
                self.nav(cx, |t, cx| t.open_on(site, id, cx));
                // LIVO_PIP：进直播间几秒后打开画中画
                if std::env::var_os("LIVO_PIP").is_some() {
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(Duration::from_secs(8)).await;
                        this.update(cx, |this, cx| {
                            this.toggle_pip(cx);
                        })
                        .ok();
                    })
                    .detach();
                }
            }
            ("settings", tab, _) => {
                self.open_settings(Some(tab.and_then(|t| t.parse().ok()).unwrap_or(0)), cx);
            }
            _ => {}
        }
    }

    /// 当前所在的位置，给后退 / 前进用
    fn here(&self) -> Loc {
        Loc {
            tab: self.tab,
            all: self.all,
            site: self.site().id(),
            sub: self.sub.clone(),
            // 搜索页记已经搜过的词，不是输入框里正在打的
            keyword: match (&self.source, self.tab) {
                (Source::Search(k), Tab::Search) => k.clone(),
                _ => String::new(),
            },
            room: self.room.as_ref().map(|r| (self.site().id(), r.detail.room_id.clone())),
        }
    }

    /// 用户主动的跳转：先记下当前位置，清掉「前进」
    fn nav(&mut self, cx: &mut Context<Self>, to: impl FnOnce(&mut Self, &mut Context<Self>)) {
        let h = self.here();
        if self.back_stack.last() != Some(&h) {
            self.back_stack.push(h);
            if self.back_stack.len() > 50 {
                self.back_stack.remove(0);
            }
        }
        self.fwd_stack.clear();
        to(self, cx);
    }

    /// 回到记下的位置（不再记录）
    fn restore(&mut self, loc: Loc, cx: &mut Context<Self>) {
        if let Some((site, room_id)) = loc.room {
            self.all = loc.all;
            return self.open_on(site, room_id, cx);
        }
        if let Some(i) = self.site_index(loc.site).filter(|&i| i != self.site_idx) {
            self.site_idx = i;
            self.cats.clear();
        }
        self.all = loc.all;
        self.keyword = loc.keyword;
        self.marked = None;
        self.go(loc.tab, cx);
        if let Some(sub) = loc.sub {
            self.open_sub(sub, cx);
        }
    }

    fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    fn can_go_forward(&self) -> bool {
        !self.fwd_stack.is_empty()
    }

    /// 后退（⌘[、工具栏、Esc）
    fn back(&mut self, cx: &mut Context<Self>) {
        let Some(loc) = self.back_stack.pop() else { return };
        self.fwd_stack.push(self.here());
        self.restore(loc, cx);
    }

    /// 前进（⌘]、工具栏）
    fn forward(&mut self, cx: &mut Context<Self>) {
        let Some(loc) = self.fwd_stack.pop() else { return };
        self.back_stack.push(self.here());
        self.restore(loc, cx);
    }

    /// 切到某个页面
    fn go(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.room.is_some() {
            self.close_room(cx);
        }
        self.ctx_menu = None;
        self.page_gen += 1;
        self.images = RetainAllImageCache::new(cx);
        if tab != Tab::Search {
            self.search_active = false;
        }
        self.tab = tab;
        // 搜索结果不算“上次浏览的平台页面”
        if tab.is_site_page() && tab != Tab::Search {
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
            Tab::History | Tab::About => {}
        }
        cx.notify();
    }

    /// 播放/停止切换；帧循环在播放器销毁（发送端关闭）后把最后一帧移出纹理图集。
    fn play(&mut self, cx: &mut Context<Self>) {
        let (volume, muted, live) = (self.volume, self.muted, self.store.live.clone());
        // 未开播的房间没有播放地址
        let Some(room) = self.room.as_mut().filter(|r| !r.urls.is_empty()) else { return };
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
                    v.playing_since = Some(Instant::now());
                    v.last_frame = None;
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
                            v.last_frame = Some(Instant::now());
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
        self.anchors.clear();
        self.anchors_all = false;
        self.page = 0;
        self.has_more = false;
        self.load_more(cx);
    }

    /// 这一页要请求的平台：推荐和搜索在「全部」时是全部平台，分区总是当前平台
    fn sources(&self) -> Vec<Arc<Site>> {
        if self.all && !matches!(self.source, Source::Category(_)) {
            self.sites.clone()
        } else {
            vec![self.site()]
        }
    }

    fn load_more(&mut self, cx: &mut Context<Self>) {
        let (sites, page, source) = (self.sources(), self.page + 1, self.source.clone());
        let merged = sites.len() > 1;
        let job = async move {
            let mut set = tokio::task::JoinSet::new();
            for (order, site) in sites.into_iter().enumerate() {
                let source = source.clone();
                set.spawn(async move { (order, site.id(), Livo::fetch_page(&site, &source, page).await) });
            }
            let mut ok = vec![];
            let mut first_err = None;
            while let Some(r) = set.join_next().await {
                match r? {
                    (order, id, Ok(v)) => ok.push((order, id, v)),
                    (_, _, Err(e)) => first_err = first_err.or(Some(e)),
                }
            }
            // 任一平台成功就显示成功的部分；全失败才报错
            if ok.is_empty() {
                return Err(first_err.unwrap_or_else(|| anyhow::anyhow!("没有可用的平台")));
            }
            ok.sort_by_key(|(order, ..)| *order);
            anyhow::Ok(ok.into_iter().map(|(_, id, v)| (id, v)).collect::<Vec<_>>())
        };
        self.fetch(cx, job, move |this, pages: Vec<(&'static str, (RoomPage, Vec<AnchorItem>))>, _| {
            // 第一页到达时从骨架屏淡入成内容；加载更多不动
            if page == 1 {
                this.page_gen += 1;
                this.anchors = pages.iter().flat_map(|(id, (_, a))| a.iter().map(|a| (*id, a.clone()))).collect();
                // 合并时在播的主播排前面
                this.anchors.sort_by_key(|(_, a)| !a.live);
            }
            this.page = page;
            this.has_more = pages.iter().any(|(_, (p, _))| p.has_more && !p.items.is_empty());
            let mut batch: Vec<(&'static str, RoomItem)> = pages.into_iter().flat_map(|(id, (p, _))| p.items.into_iter().map(move |r| (id, r))).collect();
            // 三个平台合在一起时按人气排序（每次加载的这一批内部排序，已显示的不动）
            if merged {
                batch.sort_by_key(|(_, r)| std::cmp::Reverse(r.online));
            }
            // 分页接口偶尔重叠，去重以免卡片 id 冲突
            for (id, r) in batch {
                if !this.rooms.iter().any(|(i, x)| *i == id && x.room_id == r.room_id) {
                    this.rooms.push((id, r));
                }
            }
        });
    }

    fn open_sub(&mut self, sub: SubCategory, cx: &mut Context<Self>) {
        self.page_gen += 1;
        self.sub = Some(sub.clone());
        self.load_rooms(Source::Category(sub), cx);
    }

    fn open_room(&mut self, room_id: String, cx: &mut Context<Self>) {
        let site = self.site();
        let level = self.store.live.quality_level;
        let memory = self.store.live.quality_memory.get(site.id()).cloned();
        let job = async move {
            let detail = site.room_detail(&room_id).await?;
            // 未开播也进直播间（能看简介、关注、看弹幕），只是不取播放地址
            if !detail.live {
                return Ok((detail, vec![], 0, livo_core::PlayUrl::default()));
            }
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
            let livo = cx.entity().downgrade();
            let chat = cx.new(|_| Chat { messages: vec![], list: ListState::new(0, ListAlignment::Bottom, px(200.)).measure_all(), pending: vec![], paused: false, hovering: false, connected: None, livo });
            this.menu = None;
            this.stall_retries = 0;
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
                        return;
                    }
                }
                // 弹幕连接断开（不会自动重连）：头部状态变成「已断开」，消息流里留一条系统消息
                this.update(cx, |this, cx| {
                    if let Some(room) = this.room.as_ref().filter(|r| r.id == id) {
                        room.chat.update(cx, |c, cx| c.set_connected(false, cx))
                    }
                })
                .ok();
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
        if let Some(h) = self.pip.take() {
            h.update(cx, |_, window, _| window.remove_window()).ok();
        }
        self.exit_gen += 1;
        self.page_gen += 1;
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

    /// 调试信息面板：打开时每秒从播放器读一次
    fn toggle_stats(&mut self, cx: &mut Context<Self>) {
        self.show_stats = !self.show_stats;
        self.stats_gen += 1;
        cx.notify();
        if !self.show_stats {
            return;
        }
        let gen = self.stats_gen;
        cx.spawn(async move |this, cx| loop {
            let alive = this.update(cx, |this, cx| {
                if this.stats_gen != gen {
                    return false;
                }
                this.stats = this.room.as_ref().and_then(|r| r.player.as_ref()).map(|p| p.stats()).unwrap_or_default();
                cx.notify();
                true
            });
            if !matches!(alive, Ok(true)) {
                break;
            }
            cx.background_executor().timer(Duration::from_secs(1)).await;
        })
        .detach();
    }

    fn player(&self) -> Option<&mpv::Player> {
        self.room.as_ref().and_then(|r| r.player.as_ref())
    }

    /// 暂停/继续：播放中只暂停 mpv，缓存照常下载，继续播放就是时移；没在播放时开始播放
    fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.player() else { return self.play(cx) };
        let pause = !p.paused();
        p.set_pause(pause);
        if !pause {
            // 暂停期间没有新画面，重新计时，免得看门狗立刻判成断流
            self.video.update(cx, |v, _| v.last_frame = Some(Instant::now()));
        }
        cx.notify();
    }

    /// 时移跳转到 `t` 秒，限制在已缓存的范围内；末尾留 1 秒，免得跳到缓存外面卡住
    fn seek_abs(&mut self, t: f64, cx: &mut Context<Self>) {
        let Some(p) = self.player() else { return };
        let Some((start, end, _)) = p.timeshift() else { return };
        p.seek_to(t.clamp(start, (end - 1.).max(start)));
        self.timeshift = p.timeshift();
        cx.notify();
    }

    /// 时移滑块：按比例跳到缓存范围里的位置
    fn seek_frac(&mut self, f: f32, cx: &mut Context<Self>) {
        if let Some((start, end, _)) = self.timeshift {
            self.seek_abs(start + (end - start) * f as f64, cx)
        }
    }

    fn seek_by(&mut self, secs: f64, cx: &mut Context<Self>) {
        if let Some((_, _, pos)) = self.player().and_then(|p| p.timeshift()) {
            self.seek_abs(pos + secs, cx)
        }
    }

    /// 回到直播：跳到缓存最新处并继续播放
    fn go_live(&mut self, cx: &mut Context<Self>) {
        self.seek_abs(f64::MAX, cx);
        if self.player().is_some_and(|p| p.paused()) {
            self.toggle_pause(cx)
        }
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
        // 拖动滑块时每次鼠标移动都会进来，取整后大多没变，不必每次写文件
        if style == self.store.danmaku {
            return;
        }
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
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            this.update(cx, |this, cx| {
                for (site, room_id, detail) in done {
                    let Some(d) = detail else { continue };
                    // 按 (平台, 房间号) 回写，查询期间取消的关注不会被写回来
                    let refresh = |f: &mut Follow| {
                        f.title = d.title.clone();
                        f.cover = d.cover.clone();
                        f.face = d.user_avatar.clone();
                        f.user_name = d.user_name.clone();
                        if d.live {
                            f.last_live = now;
                        }
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

    /// 当前直播间的关注记录
    fn room_follow(&self) -> Option<Follow> {
        let d = &self.room.as_ref()?.detail;
        Some(Follow { site: self.site().id().to_string(), room_id: d.room_id.clone(), user_name: d.user_name.clone(), face: d.user_avatar.clone(), title: d.title.clone(), cover: d.cover.clone(), ..Default::default() })
    }

    fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        let Some(f) = self.room_follow() else { return };
        let on = !self.store.is_followed(&f.site, &f.room_id);
        self.set_followed(f, on, cx);
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

    /// 单个平台的一页：搜索第一页同时搜主播（同原项目的两个接口，不让用户选类型）；
/// 两个接口任一成功就显示成功的部分，都失败才报错（比如斗鱼对匿名搜索要求人机验证时）
async fn fetch_page(site: &Site, source: &Source, page: u32) -> anyhow::Result<(RoomPage, Vec<AnchorItem>)> {
    Ok(match source {
        Source::Recommend => (site.recommend_rooms(page).await?, vec![]),
        Source::Category(sub) => (site.category_rooms(sub, page).await?, vec![]),
        Source::Search(kw) if page == 1 => match tokio::join!(site.search_rooms(kw, page), site.search_anchors(kw, 1)) {
            (Err(e), Err(_)) => return Err(e),
            (rooms, anchors) => (rooms.unwrap_or_default(), anchors.map(|(_, a)| a).unwrap_or_default()),
        },
        Source::Search(kw) => (site.search_rooms(kw, page).await?, vec![]),
    })
}

/// 播放页设置的快捷键动作
    fn run_action(&mut self, action: &str, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            "fullscreen" => window.toggle_fullscreen(),
            "danmaku" => self.toggle_danmaku(cx),
            "mute" => self.set_volume(self.volume, !self.muted, cx),
            "stats" => self.toggle_stats(cx),
            "refresh" => self.refresh(cx),
            "chat" => {
                self.show_chat = !self.show_chat;
                cx.notify()
            }
            "volume_up" => self.set_volume(self.volume.saturating_add(5), false, cx),
            "volume_down" => self.set_volume(self.volume.saturating_sub(5), false, cx),
            "pause" => self.toggle_pause(cx),
            "seek_back" => self.seek_by(-10., cx),
            "seek_forward" => self.seek_by(10., cx),
            _ => {}
        }
    }

    /// 刷新（⌘R）：直播间里重新取播放地址，其余页面重新加载当前内容
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = self.room.as_ref() {
            // 未开播的房间刷新就是重新进一次，看主播开播了没有
            return if r.urls.is_empty() { self.open_room(r.detail.room_id.clone(), cx) } else { self.choose_quality(r.quality_idx, cx) };
        }
        self.error = None;
        match self.tab {
            Tab::Category if self.sub.is_some() => self.load_rooms(self.source.clone(), cx),
            Tab::Category => {
                self.cats.clear();
                self.go(Tab::Category, cx)
            }
            Tab::Recommend | Tab::Search | Tab::Follow => self.go(self.tab, cx),
            Tab::History | Tab::About => cx.notify(),
        }
    }

    /// 离开直播间（Esc、菜单）：有上一页就后退，否则只关掉直播间
    fn leave_room(&mut self, cx: &mut Context<Self>) {
        if self.can_go_back() {
            self.back(cx)
        } else {
            self.close_room(cx)
        }
    }

    /// 清空搜索并返回上一页（搜索框的清除按钮、Esc）
    fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.keyword.clear();
        self.marked = None;
        self.search_active = false;
        if self.tab == Tab::Search && self.can_go_back() {
            self.back(cx)
        } else {
            cx.notify()
        }
    }

    /// 窗口底部的 Toast：可撤销的短操作直接执行，用它告诉结果
    fn show_toast(&mut self, msg: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.toast = Some(msg.into());
        self.toast_gen += 1;
        let gen = self.toast_gen;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            this.update(cx, |this, cx| {
                if this.toast_gen == gen {
                    this.toast = None;
                    cx.notify()
                }
            })
            .ok();
        })
        .detach();
    }

    fn copy_room_link(&mut self, cx: &mut Context<Self>) {
        let Some(room) = self.room.as_ref() else { return };
        let url = room_url(self.site().id(), &room.detail.room_id);
        cx.write_to_clipboard(ClipboardItem::new_string(url));
        self.show_toast("已复制直播间链接", cx);
    }

    /// 清空观看记录不能撤销：先弹系统警告框确认，默认按钮是「取消」
    fn confirm_clear_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let answer = window.prompt(
            PromptLevel::Warning,
            "清空全部观看记录？",
            Some("清空后不能恢复，关注的主播不受影响。"),
            &[PromptButton::Cancel("取消".into()), PromptButton::Other("清空记录".into())],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(1) {
                this.update(cx, |this, cx| {
                    this.store.history.clear();
                    this.store.save();
                    this.show_toast("已清空观看记录", cx)
                })
                .ok();
            }
        })
        .detach();
    }

    /// 编辑 > 粘贴：只取第一行，追加到搜索关键词
    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
        self.keyword.push_str(text.lines().next().unwrap_or("").trim());
        self.marked = None;
        cx.notify();
    }

    /// 输入法回调给的 UTF-16 范围换成关键词里的字节范围；没给就替换正在组字的部分，否则追加到末尾
    fn input_range(&self, range: Option<Range<usize>>) -> Range<usize> {
        range.map(|r| utf16_to_bytes(&self.keyword, r)).or(self.marked.clone()).unwrap_or(self.keyword.len()..self.keyword.len())
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // ⌘/⌃ 组合键归菜单（menus.rs），不当成单键快捷键或搜索输入
        if ev.keystroke.modifiers.platform || ev.keystroke.modifiers.control {
            return;
        }
        let key = ev.keystroke.key.as_str();
        if key == "escape" && self.ctx_menu.is_some() {
            self.ctx_menu = None;
            return cx.notify();
        }
        // 正在录入快捷键：这一下按键就是新绑定，不要再交给搜索框
        if self.capture_key(key, cx) {
            return cx.stop_propagation();
        }
        // 键盘焦点在某个控件上时，回车 / 空格是按下这个控件，不当成单键快捷键
        let control_focused = !self.focus.is_focused(window);
        // 直播间里点了搜索框（或 ⌘F）时按键交给搜索框，不当成单键快捷键
        if self.room.is_some() && !self.search_active {
            match key {
                "escape" if self.menu.is_some() => {
                    self.menu = None;
                    cx.notify()
                }
                "escape" if window.is_fullscreen() => window.toggle_fullscreen(),
                "escape" => self.leave_room(cx),
                _ if control_focused => {}
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
            "enter" if control_focused => return,
            "enter" if !self.keyword.trim().is_empty() => return self.nav(cx, |t, cx| t.go(Tab::Search, cx)),
            "escape" => return self.clear_search(cx),
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
    /// 只绘制看得见的几行；底部对齐：停在最底部时跟随新弹幕，往上翻看时不打扰。
    /// 要 measure_all，否则没测过高度的旧消息按 0 算，往上能翻的范围只有可见的几行
    list: ListState,
    /// 暂停滚动期间到达的消息，恢复时一起放进列表
    pending: Vec<Msg>,
    /// 头部的「暂停滚动」按钮
    paused: bool,
    /// 鼠标在栏内时也暂停自动滚动
    hovering: bool,
    /// None 连接中，Some(true) 实时，Some(false) 已断开
    connected: Option<bool>,
    /// 头部的「弹幕设置」打开播放器上的弹幕设置面板
    livo: gpui::WeakEntity<Livo>,
}

impl Chat {
    fn push(&mut self, m: LiveMessage, cx: &mut Context<Self>) {
        let msg = match m {
            LiveMessage::Online(_) => return,
            LiveMessage::Chat { user, text, .. } => Msg { user: user.into(), text: text.into(), price: None, system: false },
            LiveMessage::SuperChat { user, text, price } => Msg { user: user.into(), text: text.into(), price: Some(price), system: false },
        };
        if self.connected.is_none() {
            self.set_connected(true, cx);
        }
        self.add(msg, cx);
    }

    fn set_connected(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.connected == Some(on) {
            return;
        }
        self.connected = Some(on);
        let text = if on { "弹幕已连接" } else { "弹幕连接已断开，刷新（⌘R）可重新进入" };
        self.add(Msg { user: "".into(), text: text.into(), price: None, system: true }, cx);
    }

    fn add(&mut self, msg: Msg, cx: &mut Context<Self>) {
        if self.paused || self.hovering {
            self.pending.push(msg);
            // 暂停太久也只留最近的
            if self.pending.len() > MAX_DANMAKU {
                self.pending.drain(..self.pending.len() - MAX_DANMAKU);
            }
            return cx.notify();
        }
        let n = self.messages.len();
        self.messages.push(msg);
        self.list.splice(n..n, 1);
        if self.messages.len() > MAX_DANMAKU {
            self.messages.drain(..MAX_DANMAKU / 4);
            self.list.splice(0..MAX_DANMAKU / 4, 0);
        }
        cx.notify();
    }

    /// 恢复自动滚动：把暂停期间的消息放进来并滚到最新
    fn resume(&mut self, cx: &mut Context<Self>) {
        self.paused = false;
        self.hovering = false;
        for m in std::mem::take(&mut self.pending) {
            self.add(m, cx);
        }
        if let Some(last) = self.messages.len().checked_sub(1) {
            self.list.scroll_to_reveal_item(last);
        }
        cx.notify();
    }

    /// 普通弹幕：用户名 text-muted 600 + 内容，不给用户名随机上色；醒目留言：紫色头部（头像、名字、金额）+ 正文卡片；系统消息 12px text-subtle
    fn row(&self, ix: usize) -> AnyElement {
        let m = &self.messages[ix];
        if m.system {
            return div().px(px(SPACE_4)).py(px(3.)).t_callout().text_color(subtle()).child(m.text.clone()).into_any_element();
        }
        match m.price {
            Some(price) => div()
                .py(px(SPACE_1))
                .px(px(SPACE_3))
                .child(
                    div()
                        .rounded(px(RADIUS_MD))
                        .overflow_hidden()
                        .border_1()
                        .border_color(border())
                        .bg(raised())
                        .child(
                            div()
                                .px(px(10.))
                                .py(px(6.))
                                .flex()
                                .items_center()
                                .gap(px(SPACE_2))
                                .bg(accent_fill())
                                .text_color(c(pal().on_accent))
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(avatar("", &m.user, 18., false))
                                .child(div().flex_1().min_w(px(0.)).truncate().child(m.user.clone()))
                                .child(format!("¥{price}")),
                        )
                        .child(div().px(px(10.)).py(px(SPACE_2)).t_body().child(m.text.clone())),
                )
                .into_any_element(),
            None => {
                let line = format!("{}  {}", m.user, m.text);
                let name = 0..m.user.len();
                let styled = StyledText::new(line).with_highlights([(name, HighlightStyle { color: Some(muted().into()), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })]);
                div().px(px(SPACE_4)).py(px(3.)).t_body().text_color(text()).child(styled).into_any_element()
            }
        }
    }
}

impl Render for Chat {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chat = cx.entity();
        let state = match self.connected {
            None => status(false, "连接中"),
            Some(true) => status(true, "实时"),
            Some(false) => status(false, "已断开"),
        };
        let livo = self.livo.clone();
        let head = div()
            .flex_none()
            .h(px(44.))
            .pl(px(SPACE_4))
            .pr(px(SPACE_2))
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .border_b_1()
            .border_color(border())
            .child(div().t_headline().child("弹幕"))
            .child(state)
            .child(div().flex_1())
            .child(
                icon_btn("chat-pause", if self.paused { "play" } else { "pause" }, self.paused)
                    .tooltip(tip(if self.paused { "继续滚动" } else { "暂停滚动" }, None))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.paused {
                            this.resume(cx)
                        } else {
                            this.paused = true;
                            cx.notify()
                        }
                    })),
            )
            .child(icon_btn("chat-settings", "sliders-horizontal", false).tooltip(tip("弹幕设置", None)).on_click(move |_, _, cx| {
                livo.update(cx, |l, cx| {
                    l.menu = Some(Menu::Danmaku);
                    cx.notify()
                })
                .ok();
            }));
        let body = if self.messages.is_empty() {
            div().id("chat").flex_1().flex().items_center().justify_center().t_callout().text_color(subtle()).child("等待弹幕…").into_any_element()
        } else {
            list(self.list.clone(), move |ix, _, cx| chat.read(cx).row(ix)).flex_1().w_full().py(px(SPACE_2)).into_any_element()
        };
        // 暂停期间攒下的新弹幕：底部一个「回到最新」
        let latest = (!self.pending.is_empty()).then(|| {
            div().absolute().bottom(px(SPACE_3)).left_0().right_0().flex().justify_center().child(
                button("chat-latest", Btn::Normal, Some("chevron-down"), format!("回到最新 · {} 条新弹幕", self.pending.len()))
                    .rounded_full()
                    .shadow(shadow_popover(false))
                    .on_click(cx.listener(|this, _, _, cx| this.resume(cx))),
            )
        });
        div()
            .id("chat-panel")
            .size_full()
            .flex()
            .flex_col()
            .bg(sidebar())
            .border_l_1()
            .border_color(border())
            .child(head)
            // 鼠标在栏内时暂停自动滚动，离开时恢复（手动暂停的不受影响）
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovering = *hovered;
                if !*hovered && !this.paused {
                    this.resume(cx)
                }
            }))
            .child(div().relative().flex_1().min_h(px(0.)).flex().flex_col().child(body).children(latest))
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

/// 新内容淡入并轻微上移（macOS 切换侧边栏页面的手感）；id 变了才重新播放
fn fade_in(el: gpui::Div, id: impl Into<gpui::ElementId>) -> impl IntoElement {
    // 「减弱动态效果」开启时只保留透明度
    let shift = if reduce_motion() { 0. } else { 4. };
    el.with_animation(id, Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()), move |d, t| d.opacity(t).mt(px(shift * (1. - t))))
}

/// 卡片网格：按可用宽度以 card-min 自动分列，卡片等宽铺满；返回 (列数, 卡片宽)
fn grid_cols(width: f32) -> (usize, f32) {
    let cols = ((width + GAP) / (CARD_MIN + GAP)).floor().max(1.);
    (cols as usize, ((width - GAP * (cols - 1.)) / cols - 0.5).floor())
}

/// 卡片封面上的开播状态
#[derive(Clone, Copy, PartialEq)]
enum CardLive {
    /// 在播，带人气
    On(i64),
    Off,
    /// 不知道（观看记录）：不放徽标
    Unknown,
}

/// 侧栏导航项（macOS 源列表）：16px 图标或 8px 平台圆点 + 名字，可选右侧计数
fn nav_item(id: impl Into<SharedString>, leading: Option<&'static str>, dot: Option<Rgba>, label: impl Into<SharedString>, active: bool, count: Option<String>) -> gpui::Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let group = SharedString::from(format!("nav-{id}"));
    let item = div().id(id).group(group.clone()).h(px(ROW_HEIGHT)).flex_none().px(px(SPACE_3)).rounded(px(RADIUS_SM)).flex().items_center().gap(px(SPACE_2)).t_body().cursor_pointer().focusable_ring();
    let item = if active { item.bg(accent_soft()).text_color(accent()).font_weight(FontWeight::SEMIBOLD) } else { item.text_color(muted()).hover(|s| s.bg(hover()).text_color(text())) };
    item.children(leading.map(|n| icon(n, 16., if active { accent() } else { muted() }).when(!active, |i| i.group_hover(group, |s| s.text_color(text())))))
        .children(dot.map(|d| div().mx(px(SPACE_1)).size(px(8.)).flex_none().rounded_full().bg(d)))
        .child(div().flex_1().min_w(px(0.)).truncate().child(label.into()))
        .children(count.map(|n| div().flex_none().t_caption().text_color(subtle()).child(n)))
}

fn nav_label(text: &'static str) -> impl IntoElement {
    div().px(px(SPACE_3)).pt(px(SPACE_3)).pb(px(SPACE_1)).t_caption().font_weight(FontWeight::SEMIBOLD).text_color(subtle()).child(text)
}

/// 圆角列表（关注、搜索到的主播）：surface 底 + border，行间 1px 分隔
fn list_box(rows: impl IntoIterator<Item = gpui::Stateful<gpui::Div>>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(RADIUS_MD))
        .border_1()
        .border_color(border())
        .bg(surface())
        .overflow_hidden()
        .children(rows.into_iter().enumerate().map(|(i, r)| r.when(i > 0, |r| r.border_t_1().border_color(border()))))
}

impl Render for Livo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 外观设置：跟随系统时看窗口当前外观
        let light = match self.store.theme {
            Theme::Light => true,
            Theme::Dark => false,
            Theme::System => matches!(window.appearance(), gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight),
        };
        LIGHT.store(light, Ordering::Relaxed);
        #[cfg(target_os = "macos")]
        REDUCE_MOTION.store(system_reduce_motion(), Ordering::Relaxed);
        if std::mem::take(&mut self.pending_fullscreen) && !window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        // 直播间里全屏时只留视频
        let full = self.room.is_some() && window.is_fullscreen();
        if let WindowBounds::Windowed(b) = window.window_bounds() {
            self.store.window = Some([f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height)]);
        }
        let content_w = f32::from(window.viewport_size().width) - if self.show_sidebar && !full { SIDEBAR_WIDTH } else { 0. };
        self.ui_scale = if full { (f32::from(window.viewport_size().width) / 900.).max(1.) } else { 1. };
        let main = match &self.room {
            Some(room) => self.render_room(room, full, cx),
            None => self.render_browse(content_w, cx).into_any_element(),
        };
        div()
            .key_context("Livo")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev, window, cx| this.on_key(ev, window, cx)))
            // 菜单栏（menus.rs）：没注册的 action 对应的菜单项显示为灰色
            .on_action(cx.listener(|this, _: &menus::About, _, cx| this.nav(cx, |t, cx| t.go(Tab::About, cx))))
            .on_action(cx.listener(|this, _: &menus::CheckUpdate, _, cx| {
                this.nav(cx, |t, cx| t.go(Tab::About, cx));
                this.check_update(false, cx)
            }))
            .on_action(cx.listener(|this, _: &menus::Settings, _, cx| this.open_settings(None, cx)))
            .on_action(cx.listener(|this, _: &menus::ShowRecommend, _, cx| this.nav(cx, |t, cx| t.go(Tab::Recommend, cx))))
            .on_action(cx.listener(|this, _: &menus::ShowCategory, _, cx| this.nav(cx, |t, cx| t.go(Tab::Category, cx))))
            .on_action(cx.listener(|this, _: &menus::ShowFollow, _, cx| this.nav(cx, |t, cx| t.go(Tab::Follow, cx))))
            .on_action(cx.listener(|this, _: &menus::ShowHistory, _, cx| this.nav(cx, |t, cx| t.go(Tab::History, cx))))
            .on_action(cx.listener(|this, _: &menus::Refresh, _, cx| this.refresh(cx)))
            .on_action(cx.listener(|this, _: &menus::ToggleSidebar, _, cx| {
                this.show_sidebar = !this.show_sidebar;
                cx.notify()
            }))
            .on_action(|_: &menus::FocusNext, window, _| window.focus_next())
            .on_action(|_: &menus::FocusPrev, window, _| window.focus_prev())
            // 焦点环只跟着键盘走（macOS 惯例）：鼠标按下时不让被点的控件抢焦点，否则按下那一瞬间会闪一下焦点环；
            // Tab 聚焦过某个控件后再点别处，松开时把焦点还给根视图，直接打字搜索、单键快捷键照常可用
            .capture_any_mouse_down(|_, window, _| window.prevent_default())
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, window, _| window.focus(&this.focus)))
            .when(self.can_go_back(), |d| d.on_action(cx.listener(|this, _: &menus::GoBack, _, cx| this.back(cx))))
            .when(self.can_go_forward(), |d| d.on_action(cx.listener(|this, _: &menus::GoForward, _, cx| this.forward(cx))))
            .on_action(|_: &menus::ToggleFullScreen, window, _| window.toggle_fullscreen())
            .on_action(|_: &menus::Minimize, window, _| window.minimize_window())
            .on_action(|_: &menus::Zoom, window, _| window.zoom_window())
            .on_action(|_: &menus::CloseWindow, window, _| window.remove_window())
            // 每个页面（含直播间）工具栏都有搜索框
            .on_action(cx.listener(|this, _: &menus::EditPaste, _, cx| this.paste(cx)))
            .on_action(cx.listener(|this, _: &menus::FocusSearch, window, cx| {
                this.search_active = true;
                window.focus(&this.focus);
                cx.notify()
            }))
            .when(!self.keyword.is_empty(), |d| {
                d.on_action(cx.listener(|this, _: &menus::EditCopy, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(this.keyword.clone()))))
                    .on_action(cx.listener(|this, _: &menus::EditCut, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(std::mem::take(&mut this.keyword)));
                        this.marked = None;
                        cx.notify()
                    }))
            })
            .when(self.room.is_some(), |d| {
                d.on_action(cx.listener(|this, _: &menus::TogglePause, w, cx| this.run_action("pause", w, cx)))
                    .on_action(cx.listener(|this, _: &menus::SeekBack, w, cx| this.run_action("seek_back", w, cx)))
                    .on_action(cx.listener(|this, _: &menus::SeekForward, w, cx| this.run_action("seek_forward", w, cx)))
                    .on_action(cx.listener(|this, _: &menus::ToggleMute, w, cx| this.run_action("mute", w, cx)))
                    .on_action(cx.listener(|this, _: &menus::VolumeUp, w, cx| this.run_action("volume_up", w, cx)))
                    .on_action(cx.listener(|this, _: &menus::VolumeDown, w, cx| this.run_action("volume_down", w, cx)))
                    .on_action(cx.listener(|this, _: &menus::ToggleDanmaku, w, cx| this.run_action("danmaku", w, cx)))
                    .on_action(cx.listener(|this, _: &menus::ToggleChat, w, cx| this.run_action("chat", w, cx)))
                    .on_action(cx.listener(|this, _: &menus::ToggleStats, w, cx| this.run_action("stats", w, cx)))
                    .on_action(cx.listener(|this, _: &menus::ToggleFollow, _, cx| this.toggle_follow(cx)))
                    .on_action(cx.listener(|this, _: &menus::CopyLink, _, cx| this.copy_room_link(cx)))
                    .on_action(cx.listener(|this, _: &menus::CloseRoom, _, cx| this.leave_room(cx)))
                    .on_action(cx.listener(|this, _: &menus::PictureInPicture, _, cx| this.toggle_pip(cx)))
            })
            .size_full()
            .flex()
            .bg(bg())
            .text_color(text())
            .font_family(theme::FONT_UI)
            .t_body()
            .children((!full && self.show_sidebar).then(|| self.render_sidebar(cx)))
            .children(self.render_ctx_menu(false, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(main)
                    // 工具栏下沿的细进度条代替“加载中”文字
                    .children(self.loading.then(|| {
                        div().absolute().top(px(if full { 0. } else { TOOLBAR_HEIGHT - 2. })).left_0().right_0().h(px(2.)).bg(linear_gradient(90., linear_color_stop(c(pal().accent_fill & 0xFFFFFF00), 0.), linear_color_stop(accent_fill(), 1.)))
                    }))
                    .children(self.toast.clone().map(|t| {
                        // Toast：窗口底部居中，不放按钮
                        // 直播间里放在信息栏上方，不挡住关注、复制链接这些按钮
                        let bottom = if self.room.is_some() && !full { 100. } else { SPACE_6 };
                        div().absolute().bottom(px(bottom)).left_0().right_0().flex().justify_center().child(fade_in(
                            div()
                                .h(px(36.))
                                .pl(px(SPACE_3))
                                .pr(px(SPACE_4))
                                .rounded_full()
                                .bg(raised())
                                .border_1()
                                .border_color(border())
                                .shadow(shadow_popover(false))
                                .flex()
                                .items_center()
                                .gap(px(SPACE_2))
                                .child(icon("check", 16., c(pal().success)))
                                .child(t),
                            ("toast", self.toast_gen),
                        ))
                    })),
            )
    }
}

impl Livo {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let in_room = self.room.is_some();
        let on = |t: Tab| self.tab == t && !in_room;
        // 「推荐」= 三个平台合在一起；平台项 = 那个平台的推荐（分区页的平台在工具栏的分段控件里选）
        let discover = [
            nav_item("推荐", Some("compass"), None, "推荐", on(Tab::Recommend) && self.all, None).on_click(cx.listener(|this, _, _, cx| {
                this.nav(cx, |t, cx| {
                    t.all = true;
                    t.go(Tab::Recommend, cx)
                })
            })),
            nav_item("分区", Some("layout-grid"), None, "分区", on(Tab::Category), None).on_click(cx.listener(|this, _, _, cx| this.nav(cx, |t, cx| t.go(Tab::Category, cx)))),
        ];
        let sites = self.sites.iter().enumerate().map(|(i, s)| {
            let current = on(Tab::Recommend) && !self.all && i == self.site_idx;
            nav_item(SharedString::from(format!("site{i}")), None, Some(platform(s.id()).0), s.name(), current, None).on_click(cx.listener(move |this, _, _, cx| {
                this.nav(cx, |t, cx| {
                    t.tab = Tab::Recommend;
                    t.set_filter(Some(i), cx)
                })
            }))
        });
        let live = self.follow_list.iter().filter(|e| e.live == Some(true)).count();
        let mine = [
            nav_item("关注", Some("heart"), None, "关注", on(Tab::Follow), (live > 0).then(|| format!("{live} 在播"))).on_click(cx.listener(|this, _, _, cx| this.nav(cx, |t, cx| t.go(Tab::Follow, cx)))),
            nav_item("历史", Some("history"), None, "历史", on(Tab::History), None).on_click(cx.listener(|this, _, _, cx| this.nav(cx, |t, cx| t.go(Tab::History, cx)))),
        ];
        let update = matches!(self.update, about::Update::Available { .. } | about::Update::Ready { .. });
        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(sidebar())
            .border_r_1()
            .border_color(border())
            .px(px(SPACE_2))
            .pb(px(SPACE_3))
            // 红绿灯在侧栏顶部，与工具栏同高；双击按系统设置缩放窗口
            .child(div().id("sidebar-top").h(px(TOOLBAR_HEIGHT - SPACE_3)).flex_none().on_click(|ev, window, _| {
                if ev.click_count() == 2 {
                    window.titlebar_double_click()
                }
            }))
            // 组合标识：浅色用标准彩色版，深色用反白版；宽 96 是设计系统规定的最小屏幕宽度
            .child(
                div().px(px(SPACE_3)).pt(px(SPACE_2)).pb(px(SPACE_1)).flex_none().child(
                    img(gpui::ImageSource::Resource(gpui::Resource::Embedded(
                        if LIGHT.load(Ordering::Relaxed) { "livo-lockup.svg" } else { "livo-lockup-reverse.svg" }.into(),
                    )))
                    .w(px(96.))
                    .h(px(39.17)),
                ),
            )
            .child(nav_label("发现"))
            .child(div().flex().flex_col().gap_0p5().children(discover))
            .child(nav_label("平台"))
            .child(div().flex().flex_col().gap_0p5().children(sites))
            .child(nav_label("我的"))
            .child(div().flex().flex_col().gap_0p5().children(mine))
            .child(div().flex_1())
            .children(update.then(|| nav_item("update", Some("info"), None, "有新版本", self.tab == Tab::About && !in_room, None).on_click(cx.listener(|this, _, _, cx| this.nav(cx, |t, cx| t.go(Tab::About, cx))))))
    }

    /// 统一工具栏（标题栏与工具栏合一）：后退、页面标题、页面操作、搜索框、刷新
    fn render_toolbar(&self, title: SharedString, sub: Option<SharedString>, actions: Vec<AnyElement>, search: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let back = if self.can_go_back() { icon_btn("back", "chevron-left", false).tooltip(tip("后退", Some("⌘["))).on_click(cx.listener(|this, _, _, cx| this.back(cx))) } else { icon_btn_disabled("back", "chevron-left") };
        let forward = if self.can_go_forward() { icon_btn("forward", "chevron-right", false).tooltip(tip("前进", Some("⌘]"))).on_click(cx.listener(|this, _, _, cx| this.forward(cx))) } else { icon_btn_disabled("forward", "chevron-right") };
        // 侧栏收起时红绿灯落在工具栏左端，给它让出位置
        let left = if self.show_sidebar { SPACE_3 } else { 84. };
        div()
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
            .pl(px(left))
            .pr(px(SPACE_4))
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .border_b_1()
            .border_color(border())
            .child(div().flex().flex_none().gap(px(SPACE_1)).child(back).child(forward))
            .child(
                div()
                    .id("toolbar-title")
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .pl(px(SPACE_1))
                    .flex()
                    .items_center()
                    .gap(px(SPACE_2))
                    .on_click(|ev, window, _| {
                        if ev.click_count() == 2 {
                            window.titlebar_double_click()
                        }
                    })
                    .child(div().min_w(px(0.)).t_title3().font_weight(FontWeight::BOLD).truncate().child(title))
                    .children(sub.map(|s| div().flex_none().t_callout().text_color(muted()).child(s))),
            )
            .children(actions)
            .children(search.then(|| self.render_search_field(cx)))
            .child(icon_btn("refresh", "rotate-cw", false).tooltip(tip("刷新", Some("⌘R"))).on_click(cx.listener(|this, _, _, cx| this.refresh(cx))))
    }

    /// 平台分段控件：推荐和搜索有「全部」，分区只能选单个平台
    fn render_platform_seg(&self, cx: &mut Context<Self>) -> AnyElement {
        let with_all = self.tab != Tab::Category;
        let mut options: Vec<SharedString> = self.sites.iter().map(|s| SharedString::from(s.name())).collect();
        if with_all {
            options.insert(0, "全部".into());
        }
        let selected = if with_all && self.all { 0 } else { self.site_idx + with_all as usize };
        self.seg(
            "platform",
            options,
            selected,
            move |this, i, cx| {
                let site = if with_all { i.checked_sub(1) } else { Some(i) };
                this.nav(cx, |t, cx| t.set_filter(site, cx))
            },
            cx,
        )
        .into_any_element()
    }

    /// 在鼠标位置弹出菜单（右键菜单、设置里的下拉）
    fn open_ctx(&mut self, pos: gpui::Point<Pixels>, entries: Vec<MenuEntry<Act>>, settings: bool, cx: &mut Context<Self>) {
        self.ctx_menu = Some(CtxMenu { pos, entries, settings });
        cx.notify()
    }

    /// 画当前弹出的菜单：贴着鼠标位置、不超出窗口；点外面关掉
    fn render_ctx_menu(&self, settings: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let m = self.ctx_menu.as_ref().filter(|m| m.settings == settings)?;
        let entries = m
            .entries
            .iter()
            .map(|e| match e {
                MenuEntry::Item { label, icon, key, checked, danger, action } => MenuEntry::Item { label: label.clone(), icon: *icon, key: *key, checked: *checked, danger: *danger, action: action.clone() },
                MenuEntry::Sep => MenuEntry::Sep,
            })
            .collect();
        let this = cx.entity().downgrade();
        let close = cx.listener(|this, _, _, cx| {
            this.ctx_menu = None;
            cx.notify()
        });
        let body = menu(entries, move |act: Act, window, cx| {
            this.update(cx, |this, cx| {
                this.ctx_menu = None;
                act(this, window, cx);
                cx.notify()
            })
            .ok();
        })
        .on_mouse_down_out(close);
        Some(deferred(anchored().position(m.pos).snap_to_window_with_margin(px(SPACE_2)).child(body)).with_priority(1).into_any_element())
    }

    /// 搜索框：工具栏里 28px 高，聚焦时描边和放大镜变 accent；右侧提示 ⌘F，输入中换成 ⏎，出了结果换成清除按钮
    fn render_search_field(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.search_active || self.tab == Tab::Search || !self.keyword.is_empty();
        let searched = self.tab == Tab::Search && matches!(&self.source, Source::Search(k) if k == self.keyword.trim());
        let field = if self.keyword.is_empty() {
            div().text_color(subtle()).child("搜索直播间或主播")
        } else {
            // 输入法组字中的部分加下划线
            let underline = HighlightStyle { underline: Some(UnderlineStyle { thickness: px(1.), color: Some(accent().into()), wavy: false }), ..Default::default() };
            let styled = StyledText::new(format!("{}▏", self.keyword)).with_highlights(self.marked.clone().map(|r| (r, underline)));
            div().text_color(text()).child(styled)
        };
        let (entity, focus) = (cx.entity(), self.focus.clone());
        div()
            .id("search")
            .relative()
            .w(px(240.))
            .h(px(theme::CONTROL_MD))
            .flex_none()
            .px(px(SPACE_3))
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .rounded(px(RADIUS_SM))
            .bg(surface())
            .border_1()
            .border_color(if active { accent() } else { border_strong() })
            .cursor_text()
            .on_click(cx.listener(|this, _, window, cx| {
                this.search_active = true;
                window.focus(&this.focus);
                cx.notify()
            }))
            // 把文字输入（含输入法组字）接到这个搜索框
            .child(canvas(|_, _, _| {}, move |bounds, _, window, cx| window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx)).absolute().size_full())
            .child(icon("search", 16., if active { accent() } else { subtle() }))
            .child(field.flex_1().min_w(px(0.)).truncate())
            .child(if searched {
                div()
                    .id("search-clear")
                    .size(px(20.))
                    .flex_none()
                    .rounded(px(RADIUS_XS))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(hover()))
                    .tooltip(tip("清除搜索", Some("Esc")))
                    .child(icon("x", 14., muted()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.clear_search(cx)
                    }))
                    .into_any_element()
            } else {
                // lv-kbd 在搜索框里用 border 描边（Tooltip 里才是 border-strong）
                kbd(if self.keyword.is_empty() { "⌘F" } else { "⏎" }).border_color(border()).into_any_element()
            })
    }

    /// 窗口内提示条：放在工具栏下方，不遮挡播放画面；问题解决前一直显示
    fn render_banner(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let e = self.error.clone()?;
        let p = pal();
        let title = if self.room.is_some() { "播放出了问题" } else { "没有加载出来" };
        Some(
            div()
                .mx(px(PAD))
                .mt(px(SPACE_3))
                .flex_none()
                .px(px(SPACE_3))
                .py(px(10.))
                .rounded(px(RADIUS_MD))
                .bg(c(p.danger_soft))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(icon("circle-alert", 16., c(p.danger)))
                .child(div().flex_1().min_w(px(0.)).flex().flex_col().child(div().t_body().font_weight(FontWeight::SEMIBOLD).text_color(c(p.danger)).child(title)).child(div().t_callout().text_color(muted()).child(e)))
                .child(button("error-retry", Btn::Normal, None, "重试").on_click(cx.listener(|this, _, _, cx| this.refresh(cx))))
                .child(icon_btn("error-close", "x", false).tooltip(tip("关闭提示", None)).on_click(cx.listener(|this, _, _, cx| {
                    this.error = None;
                    cx.notify()
                }))),
        )
    }

    fn render_browse(&self, content_w: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let site = self.sites[self.site_idx].name();
        let width = content_w - 2. * PAD;
        let scope = if self.all { "全部平台" } else { site };
        let (title, sub): (String, Option<String>) = match (self.tab, &self.sub) {
            (Tab::Recommend, _) => ("推荐".into(), None),
            (Tab::Category, None) => ("分区".into(), Some(format!("{} 个大类", self.cats.len()))),
            (Tab::Category, Some(sub)) => (sub.name.clone(), Some("分区".into())),
            (Tab::Search, _) => ("搜索结果".into(), Some(scope.into())),
            (Tab::Follow, _) => ("我的关注".into(), Some(format!("{} 位主播", self.follow_list.len()))),
            (Tab::History, _) => ("观看记录".into(), Some(format!("{} 个直播间", self.store.history.len()))),
            (Tab::About, _) => ("关于 Livo".into(), None),
        };
        let mut actions = vec![];
        match self.tab {
            Tab::Recommend | Tab::Search | Tab::Category => actions.push(self.render_platform_seg(cx)),
            Tab::Follow if !self.follow_list.is_empty() => actions.push(
                self.seg(
                    "follow-sort",
                    vec!["在播优先".into(), "最近关注".into()],
                    self.follow_sort,
                    |this, i, _| this.follow_sort = i,
                    cx,
                )
                .into_any_element(),
            ),
            Tab::History if !self.store.history.is_empty() => {
                actions.push(button("history-clear", Btn::Normal, Some("trash-2"), "清空记录…").on_click(cx.listener(|this, _, window, cx| this.confirm_clear_history(window, cx))).into_any_element())
            }
            _ => {}
        }
        let body = match self.tab {
            Tab::Category if self.sub.is_none() => self.render_categories(cx).into_any_element(),
            Tab::Follow => self.render_follows(cx).into_any_element(),
            Tab::History => self.render_history(width, cx).into_any_element(),
            Tab::About => self.render_about(cx).into_any_element(),
            Tab::Search => self.render_search(width, cx).into_any_element(),
            Tab::Recommend => {
                let note = if self.all { "三个平台的推荐合在一起，按人气排序".to_string() } else { format!("{site}的推荐") };
                div().child(section_head("正在直播", Some(note))).child(self.render_grid(width, cx)).into_any_element()
            }
            _ => self.render_grid(width, cx).into_any_element(),
        };
        image_cache(self.images.clone())
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_toolbar(title.into(), sub.map(Into::into), actions, true, cx))
            .children(self.render_banner(cx))
            .child(div().id("browse").flex_1().overflow_y_scroll().p(px(PAD)).child(fade_in(div().child(body), ("page", self.page_gen))))
    }

    /// 直播间列表为空：搜索无结果在标题里回显关键词，出错时给重试
    fn render_rooms_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let retry = |label: &'static str| large(button("empty-retry", Btn::Normal, Some("rotate-cw"), label)).on_click(cx.listener(|this, _, _, cx| this.refresh(cx))).into_any_element();
        let kw = self.keyword.trim();
        match (self.tab, &self.error) {
            (Tab::Search, Some(e)) if e.contains("验证") => empty_state("triangle-alert", "搜索暂时受限", "平台要求人机验证，稍后再试，或在工具栏换个平台搜索", None),
            (Tab::Search, Some(_)) => empty_state("wifi-off", "搜索失败", "检查网络后重试", Some(retry("重新搜索"))),
            (Tab::Search, None) if kw.is_empty() => empty_state("search", "搜索直播间或主播", "直接输入关键词，按 ⏎ 搜索", None),
            (Tab::Search, None) => empty_state("search", format!("没有找到「{kw}」"), "换个关键词，或只搜主播名", None),
            (_, Some(_)) => empty_state("wifi-off", "没有加载出直播间", "检查网络后重试", Some(retry("重新加载"))),
            _ => empty_state("compass", "这里还没有直播间", "稍后刷新看看", Some(retry("刷新"))),
        }
    }

    fn render_grid(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let (cols, card_w) = grid_cols(width);
        let cover_h = card_w * 9. / 16.;
        let grid = div().flex().flex_wrap().gap(px(GAP));
        if self.rooms.is_empty() {
            return if self.loading {
                grid.children((0..cols * 2).map(|i| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(SPACE_2))
                        .child(skeleton(("sk-cover", i), card_w, cover_h, RADIUS_MD))
                        .child(skeleton(("sk-title", i), card_w * 0.8, 14., RADIUS_XS))
                        .child(skeleton(("sk-meta", i), card_w * 0.4, 12., RADIUS_XS))
                }))
                .into_any_element()
            } else {
                self.render_rooms_empty(cx).into_any_element()
            };
        }
        let cards = self.rooms.iter().map(|(site, r)| {
            let (site, id) = (*site, r.room_id.clone());
            let follow = Follow { site: site.to_string(), room_id: r.room_id.clone(), user_name: r.user_name.clone(), face: r.avatar.clone(), title: r.title.clone(), cover: r.cover.clone(), ..Default::default() };
            self.card(SharedString::from(format!("room-{site}-{}", r.room_id)), card_w, &r.cover, &r.title, &r.user_name, &r.avatar, site, CardLive::On(r.online))
                .on_click(cx.listener(move |this, _, _, cx| this.nav(cx, |t, cx| t.open_on(site, id.clone(), cx))))
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.open_ctx(ev.position, this.room_menu(follow.clone(), true, vec![]), false, cx)))
        });
        let more = self.has_more.then(|| {
            div().pt(px(SPACE_6)).flex().justify_center().child(if self.loading {
                div().h(px(theme::CONTROL_MD)).flex().items_center().t_callout().text_color(muted()).child("正在加载…").into_any_element()
            } else {
                button("more", Btn::Normal, None, "加载更多").on_click(cx.listener(|this, _, _, cx| this.load_more(cx))).into_any_element()
            })
        });
        div().flex().flex_col().child(grid.children(cards)).children(more).into_any_element()
    }

    /// 直播间卡片：16:9 封面（左上 LIVE、左下人气）+ 两行标题 + 主播名与平台标签
    #[allow(clippy::too_many_arguments)]
    fn card(&self, id: SharedString, w: f32, cover: &str, title: &str, user: &str, face: &str, site_id: &str, live: CardLive) -> gpui::Stateful<gpui::Div> {
        let offline = live == CardLive::Off;
        let site_name = self.site_index(site_id).map_or("", |i| self.sites[i].name());
        let image = img(SharedString::from(cover.to_string()))
            .size_full()
            .object_fit(ObjectFit::Cover)
            // 未开播：去色并降到 45% 不透明
            .grayscale(offline)
            .when(offline, |i| i.opacity(0.45))
            .with_loading(|| div().size_full().bg(hover()).into_any_element())
            .with_fallback(|| div().size_full().bg(hover()).flex().items_center().justify_center().child(icon("image-off", 20., subtle())).into_any_element());
        let (top_left, bottom_left) = match live {
            CardLive::On(n) => (Some(badge(Badge::Live, None, "LIVE")), Some(badge(Badge::Media, Some("users"), fmt_count(n)))),
            CardLive::Off => (Some(badge(Badge::Neutral, None, "未开播")), None),
            CardLive::Unknown => (None, None),
        };
        div()
            .id(id)
            .group("card")
            .w(px(w))
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .rounded(px(RADIUS_MD))
            .cursor_pointer()
            .focusable_ring()
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(w * 9. / 16.))
                    .rounded(px(RADIUS_MD))
                    .bg(hover())
                    // 悬停：描边变 accent 并加 shadow-card；不放大、不移动布局
                    .group_hover("card", |s| s.shadow(shadow_card()))
                    .child(div().size_full().rounded(px(RADIUS_MD)).overflow_hidden().child(image))
                    .children(top_left.map(|b| div().absolute().top(px(SPACE_2)).left(px(SPACE_2)).child(b)))
                    .children(bottom_left.map(|b| div().absolute().bottom(px(SPACE_2)).left(px(SPACE_2)).child(b)))
                    // 描边叠在封面上面：浅色封面在浅色底、深色封面在深色底都看得出边界
                    .child(div().absolute().inset_0().rounded(px(RADIUS_MD)).border_1().border_color(border()).group_hover("card", |s| s.border_color(accent()))),
            )
            // 标题最多两行，按内容高度排，单行标题下面不留空行
            .child(div().t_body().font_weight(FontWeight::SEMIBOLD).line_clamp(2).group_hover("card", |s| s.text_color(accent())).child(title.to_string()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .t_callout()
                    .text_color(muted())
                    .child(avatar(face, user, 24., false))
                    .child(div().flex_1().min_w(px(0.)).truncate().child(user.to_string()))
                    .child(platform_tag(site_id, site_name)),
            )
    }

    /// 直播间的右键菜单：进入直播间（默认操作）、关注、复制链接、在浏览器中打开；`tail` 是放在最后的破坏性操作
    fn room_menu(&self, f: Follow, with_follow: bool, tail: Vec<MenuEntry<Act>>) -> Vec<MenuEntry<Act>> {
        let followed = self.store.is_followed(&f.site, &f.room_id);
        let url = room_url(&f.site, &f.room_id);
        let site = self.site_index(&f.site).map_or("", |i| self.sites[i].id());
        let (room_id, url2) = (f.room_id.clone(), url.clone());
        let mut m: Vec<MenuEntry<Act>> = vec![MenuEntry::item("进入直播间", Some("play"), Rc::new(move |t: &mut Livo, _: &mut Window, cx: &mut Context<Livo>| t.nav(cx, |t, cx| t.open_on(site, room_id.clone(), cx))) as Act)];
        if with_follow {
            m.push(if followed {
                MenuEntry::item("取消关注", Some("heart"), Rc::new(move |t: &mut Livo, _: &mut Window, cx: &mut Context<Livo>| t.set_followed(f.clone(), false, cx)) as Act)
            } else {
                MenuEntry::item("关注", Some("plus"), Rc::new(move |t: &mut Livo, _: &mut Window, cx: &mut Context<Livo>| t.set_followed(f.clone(), true, cx)) as Act)
            });
        }
        m.extend([
            MenuEntry::Sep,
            MenuEntry::item(
                "复制链接",
                Some("copy"),
                Rc::new(move |t: &mut Livo, _: &mut Window, cx: &mut Context<Livo>| {
                    cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
                    t.show_toast("已复制直播间链接", cx)
                }) as Act,
            )
            .key("⇧⌘C"),
            MenuEntry::item("在浏览器中打开", Some("external-link"), Rc::new(move |_: &mut Livo, _: &mut Window, cx: &mut Context<Livo>| cx.open_url(&url2)) as Act),
        ]);
        if !tail.is_empty() {
            m.push(MenuEntry::Sep);
            m.extend(tail);
        }
        m
    }

    /// 直播间信息栏的「更多」菜单
    fn room_more_menu(&self) -> Vec<MenuEntry<Act>> {
        let act = |f: fn(&mut Livo, &mut Window, &mut Context<Livo>)| Rc::new(f) as Act;
        let url = self.room.as_ref().map(|r| room_url(self.site().id(), &r.detail.room_id)).unwrap_or_default();
        vec![
            MenuEntry::item("在浏览器中打开", Some("external-link"), Rc::new(move |_: &mut Livo, _: &mut Window, cx: &mut Context<Livo>| cx.open_url(&url)) as Act),
            MenuEntry::Sep,
            MenuEntry::item(if self.pip.is_some() { "退出画中画" } else { "画中画" }, Some("picture-in-picture-2"), act(|t, _, cx| t.toggle_pip(cx))).key("⌥⌘P"),
            MenuEntry::item(if self.show_chat { "收起弹幕栏" } else { "展开弹幕栏" }, Some("message-square-text"), act(|t, _, cx| {
                t.show_chat = !t.show_chat;
                cx.notify()
            }))
            .key("⌥⌘D"),
            MenuEntry::item(if self.show_stats { "隐藏统计信息" } else { "显示统计信息" }, Some("activity"), act(|t, _, cx| t.toggle_stats(cx))),
        ]
    }

    /// 关注 / 取消关注某个直播间（直播间里的按钮、各处右键菜单），结果用 Toast 告诉
    fn set_followed(&mut self, f: Follow, on: bool, cx: &mut Context<Self>) {
        let msg = if on {
            if !self.store.is_followed(&f.site, &f.room_id) {
                self.store.follows.push(f.clone());
            }
            format!("已关注 {}", f.user_name)
        } else {
            self.store.follows.retain(|x| !(x.site == f.site && x.room_id == f.room_id));
            self.follow_list.retain(|e| !(e.follow.site == f.site && e.follow.room_id == f.room_id));
            format!("已取消关注 {}", f.user_name)
        };
        self.store.save();
        self.show_toast(msg, cx);
    }

    /// 主播行（关注列表、搜索结果）：头像 → 名字 + 平台 / 直播标题 → 状态与数字 → 箭头
    #[allow(clippy::too_many_arguments)]
    fn anchor_row(&self, id: SharedString, avatar_url: &str, name: &str, site_id: &str, title: &str, live: Option<bool>, meta: Option<String>) -> gpui::Stateful<gpui::Div> {
        let site_name = self.site_index(site_id).map_or("", |i| self.sites[i].name());
        let state = match live {
            Some(true) => status(true, "直播中"),
            Some(false) => status(false, "未开播"),
            None => status(false, "查询中"),
        };
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(SPACE_3))
            .px(px(SPACE_4))
            .py(px(10.))
            .cursor_pointer()
            .hover(|s| s.bg(hover()))
            .focusable_ring()
            .child(avatar(avatar_url, name, 40., live == Some(true)))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().flex().items_center().gap(px(SPACE_2)).child(div().min_w(px(0.)).t_headline().truncate().child(name.to_string())).child(platform_tag(site_id, site_name)))
                    .children((!title.is_empty()).then(|| div().t_callout().text_color(muted()).truncate().child(title.to_string()))),
            )
            .child(div().flex_none().flex().flex_col().items_end().gap(px(2.)).child(state).children(meta.map(|m| div().t_callout().text_color(muted()).child(m))))
            .child(icon("chevron-right", 16., subtle()))
    }

    /// 搜索结果：「主播」列表在上（最多 3 行，可展开），「直播间」网格在下
    fn render_search(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = if self.anchors_all { self.anchors.len() } else { self.anchors.len().min(3) };
        let rows = self.anchors[..shown].iter().enumerate().map(|(i, (site, a))| {
            let (site, id) = (*site, a.room_id.clone());
            let follow = Follow { site: site.to_string(), room_id: a.room_id.clone(), user_name: a.user_name.clone(), face: a.avatar.clone(), title: String::new(), cover: String::new(), ..Default::default() };
            self.anchor_row(SharedString::from(format!("anchor-{i}")), &a.avatar, &a.user_name, site, "", Some(a.live), None)
                .on_click(cx.listener(move |this, _, _, cx| this.nav(cx, |t, cx| t.open_on(site, id.clone(), cx))))
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _, cx| this.open_ctx(ev.position, this.room_menu(follow.clone(), true, vec![]), false, cx)))
        });
        let toggle = (self.anchors.len() > 3).then(|| {
            let label = if self.anchors_all { "收起".to_string() } else { format!("显示全部 {} 位", self.anchors.len()) };
            button("anchors-all", Btn::Ghost, None, label).on_click(cx.listener(|this, _, _, cx| {
                this.anchors_all = !this.anchors_all;
                cx.notify()
            }))
        });
        let anchors = (!self.anchors.is_empty()).then(|| {
            div()
                .max_w(px(820.))
                .mb(px(SPACE_6))
                .flex()
                .flex_col()
                .child(div().flex().items_start().justify_between().child(section_head("主播", Some(self.anchors.len().to_string()))).children(toggle))
                .child(list_box(rows))
        });
        let rooms_head = (!self.rooms.is_empty() || !self.anchors.is_empty()).then(|| section_head("直播间", (!self.rooms.is_empty()).then(|| self.rooms.len().to_string())));
        div().flex().flex_col().children(anchors).children(rooms_head).child(self.render_grid(width, cx))
    }

    /// 我的关注：「直播中」在上（按人气），「未开播」在下；列表不铺满宽屏
    fn render_follows(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.follow_list.is_empty() {
            let go = large(button("go-recommend", Btn::Primary, None, "去看推荐")).on_click(cx.listener(|this, _, _, cx| {
                this.nav(cx, |t, cx| {
                    t.all = true;
                    t.go(Tab::Recommend, cx)
                })
            }));
            return empty_state("heart", "还没有关注的主播", "进入直播间点「关注」，开播后会排在这里", Some(go.into_any_element())).into_any_element();
        }
        let row = |i: usize| {
            let e = &self.follow_list[i];
            let f = e.follow.clone();
            let meta = match e.live {
                Some(true) => Some(format!("{} 人气", fmt_count(e.online))),
                _ if f.last_live > 0 => Some(format!("{}开播过", settings::time_ago(f.last_live))),
                _ => None,
            };
            let menu_f = f.clone();
            self.anchor_row(SharedString::from(format!("follow-{}-{}", f.site, f.room_id)), &f.face, &f.user_name, &f.site, &f.title, e.live, meta)
                .on_click(cx.listener(move |this, _, _, cx| this.nav(cx, |t, cx| t.open_follow(i, cx))))
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    // 关注列表里「取消关注」是破坏性操作，放在最后
                    let f = menu_f.clone();
                    let unfollow = MenuEntry::item("取消关注", Some("heart"), Rc::new(move |t: &mut Livo, _: &mut Window, cx: &mut Context<Livo>| t.set_followed(f.clone(), false, cx)) as Act).danger();
                    this.open_ctx(ev.position, this.room_menu(menu_f.clone(), false, vec![unfollow]), false, cx)
                }))
        };
        let list = |title: &'static str, items: Vec<usize>| {
            let n = items.len();
            (n > 0).then(|| div().flex().flex_col().child(section_head(title, Some(n.to_string()))).child(list_box(items.into_iter().map(row))))
        };
        let page = div().max_w(px(820.)).flex().flex_col().gap(px(SPACE_6));
        if self.follow_sort == 1 {
            // 最近关注：按关注的先后倒序，不分组
            let items: Vec<usize> = (0..self.follow_list.len()).rev().collect();
            return page.children(list("全部关注", items)).into_any_element();
        }
        // 在播优先：「直播中」按人气，「未开播」按上次开播时间
        let (mut on, mut off): (Vec<usize>, Vec<usize>) = (0..self.follow_list.len()).partition(|&i| self.follow_list[i].live == Some(true));
        on.sort_by_key(|&i| std::cmp::Reverse(self.follow_list[i].online));
        off.sort_by_key(|&i| std::cmp::Reverse(self.follow_list[i].follow.last_live));
        page.children(list("直播中", on)).children(list("未开播", off)).into_any_element()
    }

    /// 横向滑块：点击或拖动时把比例交给 `set`。width 为 None 时铺满父元素
    #[allow(clippy::too_many_arguments)]
    fn slider(&self, idx: usize, frac: f32, width: Option<f32>, track: Rgba, fill: Rgba, thumb: f32, set: fn(&mut Self, f32, &mut Context<Self>), cx: &mut Context<Self>) -> impl IntoElement {
        let bar = self.sliders[idx].clone();
        let frac = frac.clamp(0., 1.);
        let thumb_shadow = vec![
            BoxShadow { color: c(0x0000004D).into(), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(0.5) },
            BoxShadow { color: c(0x0000004D).into(), offset: point(px(0.), px(1.)), blur_radius: px(3.), spread_radius: px(0.) },
        ];
        div()
            .id(SharedString::from(format!("slider-{idx}")))
            .relative()
            .map(|d| match width {
                Some(w) => d.w(px(w)).flex_none(),
                None => d.w_full(),
            })
            .h(px(20.))
            .flex()
            .items_center()
            .cursor_pointer()
            .child(canvas(move |b, _, _| bar.set(b), |_, _, _, _| {}).absolute().size_full())
            .child(div().w_full().h(px(4.)).rounded_full().bg(track).child(div().h_full().w(relative(frac)).rounded_full().bg(fill)))
            .child(div().absolute().left(relative(frac)).ml(px(-thumb / 2.)).size(px(thumb)).rounded_full().bg(rgb(0xffffff)).shadow(thumb_shadow))
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

    /// 弹幕设置的四个滑块：(标题, 当前值, 滑块)。字号、不透明度、滚动速度、显示区域（范围同原项目，字号收窄到 12-60）
    /// `media` 为播放器里的深色面板（不跟随主题）；回调要是不捕获变量的 fn，直接在闭包里按比例换算并取整
    pub(crate) fn danmaku_sliders(&self, media: bool, cx: &mut Context<Self>) -> Vec<(&'static str, String, gpui::AnyElement)> {
        let st = self.store.danmaku;
        let p = if media { &theme::DARK } else { pal() };
        let (width, track, fill) = (if media { None } else { Some(192.) }, c(p.border_strong), c(p.accent_fill));
        vec![
            ("字体大小", format!("{}", st.font), self.slider(1, (st.font - 12.) / 48., width, track, fill, 16., |t, f, cx| t.set_danmaku_style(DanmakuStyle { font: (12. + f * 48.).round(), ..t.store.danmaku }, cx), cx).into_any_element()),
            ("不透明度", format!("{}%", (st.opacity * 100.).round()), self.slider(2, (st.opacity - 0.1) / 0.9, width, track, fill, 16., |t, f, cx| t.set_danmaku_style(DanmakuStyle { opacity: ((0.1 + f * 0.9) * 20.).round() / 20., ..t.store.danmaku }, cx), cx).into_any_element()),
            ("滚动速度", format!("{} 秒", st.secs), self.slider(3, (st.secs - 1.) / 29., width, track, fill, 16., |t, f, cx| t.set_danmaku_style(DanmakuStyle { secs: (1. + f * 29.).round(), ..t.store.danmaku }, cx), cx).into_any_element()),
            ("显示区域", format!("{}%", (st.area * 100.).round()), self.slider(4, (st.area - 0.1) / 0.9, width, track, fill, 16., |t, f, cx| t.set_danmaku_style(DanmakuStyle { area: ((0.1 + f * 0.9) * 20.).round() / 20., ..t.store.danmaku }, cx), cx).into_any_element()),
        ]
    }

    /// 播放器里弹出的弹幕设置（深色，竖排）
    fn render_danmaku_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = &theme::DARK;
        div()
            .flex()
            .flex_col()
            .px(px(SPACE_2))
            .pb(px(6.))
            .children(self.danmaku_sliders(true, cx).into_iter().map(|(label, value, slider)| {
                div().flex().flex_col().gap(px(SPACE_1)).py(px(6.)).child(div().flex().justify_between().t_callout().child(label).child(div().t_timecode().text_color(c(d.text_muted)).child(value))).child(slider)
            }))
            .child(div().pt(px(SPACE_1)).t_caption().text_color(c(d.text_subtle)).child("滚动速度为弹幕横穿画面的秒数，越小越快"))
    }

    fn render_categories(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.cats.is_empty() {
            if !self.loading && self.error.is_some() {
                return self.render_rooms_empty(cx).into_any_element();
            }
            return div().flex().flex_wrap().gap(px(SPACE_3)).children((0..24usize).map(|i| skeleton(("sk-cat", i), 120., 108., RADIUS_MD))).into_any_element();
        }
        let tabs = self.cats.iter().enumerate().map(|(i, cat)| {
            let on = i == self.cat_idx;
            let t = div().id(SharedString::from(format!("cat{i}"))).h(px(theme::CONTROL_MD)).px(px(SPACE_3)).rounded(px(RADIUS_SM)).flex().items_center().cursor_pointer();
            if on { t.bg(accent_soft()).text_color(accent()).font_weight(FontWeight::SEMIBOLD) } else { t.text_color(muted()).hover(|s| s.bg(hover()).text_color(text())) }.child(cat.name.clone()).on_click(cx.listener(move |this, _, _, cx| {
                this.cat_idx = i;
                this.touch("cats");
                cx.notify()
            }))
        });
        let subs = self.cats.get(self.cat_idx).map(|c| c.children.as_slice()).unwrap_or_default().iter().enumerate().map(|(i, s)| {
            let sub = s.clone();
            div()
                .id(SharedString::from(format!("sub{i}")))
                .w(px(120.))
                .p(px(SPACE_3))
                .rounded(px(RADIUS_MD))
                .bg(surface())
                .border_1()
                .border_color(border())
                .cursor_pointer()
                .hover(|st| st.border_color(accent()).shadow(shadow_card()))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(SPACE_2))
                .on_click(cx.listener(move |this, _, _, cx| this.open_sub(sub.clone(), cx)))
                .child(
                    img(SharedString::from(s.pic.clone()))
                        .size(px(56.))
                        .rounded(px(RADIUS_SM))
                        .object_fit(ObjectFit::Cover)
                        .with_fallback(|| div().size(px(56.)).rounded(px(RADIUS_SM)).bg(hover()).flex().items_center().justify_center().child(icon("image-off", 20., subtle())).into_any_element()),
                )
                .child(div().w_full().t_callout().text_center().truncate().child(s.name.clone()))
        });
        div()
            .flex()
            .flex_col()
            .gap(px(SPACE_4))
            .child(div().flex().flex_wrap().gap(px(SPACE_1)).children(tabs))
            .child(fade_in(div().flex().flex_wrap().gap(px(SPACE_3)).children(subs), ("cats", self.anim_gen("cats").unwrap_or(0))))
            .into_any_element()
    }

    /// 控制条是否显示：暂停或菜单打开时常驻；播放中鼠标静止 3 秒或离开画面后淡出
    fn controls_shown(&self, playing: bool) -> bool {
        !playing || self.menu.is_some() || Instant::now() < self.controls_until
    }

    /// 鼠标在画面上移动：控制条出现，3 秒后到期时重绘一次让它淡出
    fn poke_controls(&mut self, cx: &mut Context<Self>) {
        let was_hidden = Instant::now() >= self.controls_until;
        self.controls_until = Instant::now() + Duration::from_secs(3);
        if !was_hidden {
            return;
        }
        cx.notify();
        cx.spawn(async move |this, cx| loop {
            let left = this.update(cx, |this, _| this.controls_until.saturating_duration_since(Instant::now())).unwrap_or_default();
            if left.is_zero() {
                this.update(cx, |_, cx| cx.notify()).ok();
                break;
            }
            cx.background_executor().timer(left).await;
        })
        .detach();
    }

    /// 显隐切换时淡入淡出 200ms；「减弱动态效果」开启时直接切换
    fn fade(&self, el: gpui::Div, key: &'static str, shown: bool) -> AnyElement {
        if reduce_motion() {
            return el.opacity(if shown { 1. } else { 0. }).into_any_element();
        }
        el.with_animation((key, shown as usize), Animation::new(Duration::from_millis(200)).with_easing(ease_out_quint()), move |e, t| e.opacity(if shown { t } else { 1. - t })).into_any_element()
    }

    /// 控制条的高度：第一行时移（有回看范围时）+ 第二行按钮，用来把弹出菜单放在它上方
    fn controls_height(&self) -> f32 {
        16. + 32. + if self.timeshift.is_some_and(|(s, e, _)| e - s > 1.) { 26. } else { 0. }
    }

    /// 悬浮在画面底部的播放控制条：第一行时移进度，第二行 播放 / LIVE 或回看时长 / 音量 … 清晰度 / 线路 / 弹幕 / 统计 / 全屏
    fn render_controls(&self, room: &Room, full: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let d = &theme::DARK;
        let playing = room.player.as_ref().is_some_and(|p| !p.paused());
        let white = c(d.on_media);
        let key = |action: &str| settings::key_label(&self.store.key_for(action));
        let popup = |id: &'static str, label: String, open: bool| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(SPACE_2))
                .rounded(px(RADIUS_SM))
                .border_1()
                .border_color(c(0xFFFFFF4D))
                .when(open, |b| b.bg(c(0xFFFFFF24)))
                .flex()
                .items_center()
                .gap(px(SPACE_1))
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(white)
                .cursor_pointer()
                .hover(|s| s.bg(c(0xFFFFFF24)))
                .child(label)
                .child(icon("chevron-down", 14., white))
        };
        let level = if self.muted { 0. } else { self.volume as f32 / 100. };
        let volume_slider = self.slider(0, level, Some(72. * self.ui_scale), c(0xFFFFFF47), white, 12., |t, f, cx| t.set_volume((f * 100.).round() as u8, false, cx), cx);
        let quality = room.qualities.get(room.quality_idx).map_or("清晰度".to_string(), |q| q.name.clone());
        // 时移：拖动在已缓存的范围里回看；直播本身就有几秒延迟，落后不到 LIVE_EDGE 都算直播，滑块钉在最右边
        let ts = self.timeshift.filter(|(s, e, _)| e - s > 1.);
        let behind = ts.map_or(0., |(_, end, pos)| end - pos);
        let at_live = behind < LIVE_EDGE;
        let frac = match ts {
            Some((start, end, pos)) if !at_live => ((pos - start) / (end - start)) as f32,
            _ => 1.,
        };
        let ts_row = Some(div().flex().items_center().child(self.slider(5, frac, None, c(0xFFFFFF47), accent_fill(), 12., |t, f, cx| t.seek_frac(f, cx), cx)));
        let position = if at_live {
            div().mx(px(6.)).child(badge(Badge::Live, None, "LIVE")).into_any_element()
        } else {
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .mx(px(6.))
                .child(div().t_timecode().text_color(white).child(fmt_behind(behind)))
                .child(
                    div()
                        .id("go-live")
                        .h(px(22.))
                        .px(px(SPACE_2))
                        .rounded_full()
                        .bg(c(0xFFFFFF29))
                        .flex()
                        .items_center()
                        .gap(px(SPACE_1))
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(c(0xFFFFFF40)))
                        .child(icon("radio", 14., white))
                        .child("回到直播")
                        .on_click(cx.listener(|this, _, _, cx| this.go_live(cx))),
                )
                .into_any_element()
        };
        let toggle_menu = |m: Menu| {
            move |this: &mut Self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
                this.menu = if this.menu == Some(m) { None } else { Some(m) };
                cx.notify()
            }
        };
        let row = div()
            .flex()
            .items_center()
            .gap(px(SPACE_1))
            .child(media_btn("play", if playing { "pause" } else { "play" }, false).tooltip(tip(if playing { "暂停" } else { "播放" }, Some(&key("pause")))).on_click(cx.listener(|this, _, _, cx| this.toggle_pause(cx))))
            .child(position)
            .child(
                media_btn("mute", if self.muted || self.volume == 0 { "volume-x" } else if self.volume < 50 { "volume-1" } else { "volume-2" }, false)
                    .tooltip(tip(if self.muted { "取消静音" } else { "静音" }, Some(&key("mute"))))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let v = if this.volume == 0 { 50 } else { this.volume };
                        this.set_volume(v, !this.muted && this.volume > 0, cx)
                    })),
            )
            .child(volume_slider)
            .child(div().flex_1())
            .child(popup("quality", quality, self.menu == Some(Menu::Quality)).on_click(cx.listener(toggle_menu(Menu::Quality))))
            .child(popup("line", format!("线路 {}", room.line_idx + 1), self.menu == Some(Menu::Line)).on_click(cx.listener(toggle_menu(Menu::Line))))
            .child(
                media_btn("danmaku", if self.show_danmaku { "message-square-text" } else { "message-square-off" }, self.show_danmaku)
                    .tooltip(tip(if self.show_danmaku { "关闭弹幕" } else { "打开弹幕" }, Some(&key("danmaku"))))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_danmaku(cx))),
            )
            .child(media_btn("danmaku-settings", "sliders-horizontal", self.menu == Some(Menu::Danmaku)).tooltip(tip("弹幕设置", None)).on_click(cx.listener(toggle_menu(Menu::Danmaku))))
            .child(media_btn("pip", "picture-in-picture-2", self.pip.is_some()).tooltip(tip(if self.pip.is_some() { "退出画中画" } else { "画中画" }, Some("⌥⌘P"))).on_click(cx.listener(|this, _, _, cx| this.toggle_pip(cx))))
            .child(media_btn("fullscreen", if full { "minimize" } else { "maximize" }, false).tooltip(tip(if full { "退出全屏" } else { "全屏" }, Some(&key("fullscreen")))).on_click(|_, window, _| window.toggle_fullscreen()));
        let shown = self.controls_shown(playing);
        let bar = div()
            .absolute()
            .left(px(SPACE_3))
            .right(px(SPACE_3))
            .bottom(px(SPACE_3))
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(SPACE_3))
            .py(px(SPACE_2))
            .rounded(px(RADIUS_LG))
            .bg(c(d.scrim))
            .text_color(white)
            // 控件上的点击不要冒泡成视频的双击全屏
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(ts_row)
            .child(row);
        self.fade(bar, "controls", shown)
    }

    /// 视频左上角的统计信息面板
    fn render_stats(&self) -> impl IntoElement {
        let rows = self.stats.iter().map(|(k, v)| div().flex().gap(px(SPACE_3)).child(div().w(px(56.)).flex_none().text_color(c(0xFFFFFF99)).child(*k)).child(div().font_family(MONO).text_color(rgb(0xffffff)).child(v.clone())));
        div()
            .absolute()
            .top(px(SPACE_3))
            .left(px(SPACE_3))
            .max_w(px(440.))
            .px(px(SPACE_3))
            .py(px(10.))
            .rounded(px(RADIUS_MD))
            // 足够深才盖得住后面的滚动弹幕；细描边让它在黑边上也有轮廓（类似 macOS 的 HUD 面板）
            .bg(c(0x000000D9))
            .border_1()
            .border_color(c(0xFFFFFF1F))
            .text_size(px(11.))
            .line_height(px(18.))
            .flex()
            .flex_col()
            .children(rows)
    }

    /// 控制条上方弹出的菜单：清晰度、线路（当前项打勾），或弹幕设置；播放器区域永远深色
    fn render_menu(&self, room: &Room, menu: Menu, cx: &mut Context<Self>) -> impl IntoElement {
        let d = &theme::DARK;
        let (title, items, selected): (&str, Vec<(String, Option<&str>)>, usize) = match menu {
            Menu::Quality => ("清晰度", room.qualities.iter().map(|q| (q.name.clone(), None)).collect(), room.quality_idx),
            Menu::Line => ("线路", room.urls.iter().enumerate().map(|(i, u)| (format!("线路 {}", i + 1), Some(if u.contains(".flv") { "FLV" } else { "HLS" }))).collect(), room.line_idx),
            Menu::Danmaku => ("弹幕设置", vec![], 0),
        };
        let rows = items.into_iter().enumerate().map(|(i, (label, badge))| {
            let on = i == selected;
            div()
                .id(SharedString::from(format!("menu-{i}")))
                .group("menu-item")
                .h(px(24.))
                .pl(px(6.))
                .pr(px(SPACE_2))
                .rounded(px(5.))
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .cursor_pointer()
                // macOS 菜单惯例：高亮项 accent-fill 底 + on-accent 字
                .hover(|s| s.bg(c(d.accent_fill)).text_color(c(d.on_accent)))
                .on_click(cx.listener(move |this, _, _, cx| match menu {
                    Menu::Quality => this.choose_quality(i, cx),
                    Menu::Line => this.choose_line(i, cx),
                    Menu::Danmaku => {}
                }))
                .child(icon("check", 14., c(d.text)).group_hover("menu-item", |s| s.text_color(c(d.on_accent))).when(!on, |i| i.invisible()))
                .child(div().flex_1().child(label))
                .children(badge.map(|b| div().text_size(px(12.)).text_color(c(d.text_subtle)).group_hover("menu-item", |s| s.text_color(c(d.on_accent))).child(b)))
        });
        let bottom = SPACE_3 + self.controls_height() + SPACE_2;
        div()
            .id("menu")
            .absolute()
            .right(px(SPACE_3))
            .bottom(px(bottom))
            .w(px(if menu == Menu::Danmaku { 280. } else { 200. }))
            .p(px(5.))
            .rounded(px(RADIUS_MD))
            .bg(c(d.surface_raised))
            .border_1()
            .border_color(c(d.border))
            .shadow(shadow_popover(true))
            .text_size(px(13.))
            .text_color(c(d.text))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().pt(px(SPACE_1)).pb(px(2.)).pl(px(if menu == Menu::Danmaku { SPACE_2 } else { 26. })).t_caption().font_weight(FontWeight::SEMIBOLD).text_color(c(d.text_subtle)).child(title))
            .child(if menu == Menu::Danmaku {
                div().id("menu-list").child(self.render_danmaku_settings(cx))
            } else {
                div().id("menu-list").max_h(px(320.)).overflow_y_scroll().flex().flex_col().children(rows)
            })
            // 打开时淡入并轻轻上移
            .with_animation(("menu", menu as usize), Animation::new(Duration::from_millis(150)).with_easing(ease_out_quint()), move |el, t| el.opacity(t).bottom(px(bottom - 6. * (1. - t))))
    }

    fn render_room(&self, room: &Room, full: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let d = &room.detail;
        let site = self.site();
        let media = &theme::DARK;
        let offline = room.urls.is_empty();
        let playing = room.player.as_ref().is_some_and(|p| !p.paused());
        let video = div()
            .id("video")
            .group("video")
            .on_mouse_move(cx.listener(|this, _: &MouseMoveEvent, _, cx| this.poke_controls(cx)))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered {
                    this.controls_until = Instant::now();
                    cx.notify()
                }
            }))
            .relative()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .bg(rgb(0x000000))
            .overflow_hidden()
            .on_click(|ev, window, _| {
                if ev.click_count() >= 2 {
                    window.toggle_fullscreen()
                }
            })
            .child(if offline {
                // 未开播：画面位置显示头像和提示，不放播放器控件
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(SPACE_3))
                    .child(avatar(&d.user_avatar, &d.user_name, 72., false))
                    .child(div().t_title3().text_color(c(media.on_media)).child("主播当前未开播"))
                    .child(div().t_callout().text_color(c(media.text_muted)).child("可以先关注，开播后会排在「我的关注」最前面"))
                    .into_any_element()
            } else if self.pip.is_some() {
                // 画面在画中画小窗里
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(SPACE_3))
                    .child(icon("picture-in-picture-2", 20., c(media.text_muted)))
                    .child(div().t_body().text_color(c(media.text_muted)).child("正在画中画中播放"))
                    .child(media_btn_text("pip-back", "回到主窗口").on_click(cx.listener(|this, _, _, cx| this.toggle_pip(cx))))
                    .into_any_element()
            } else {
                self.video.clone().into_any_element()
            })
            // 全屏时顶部同时显示直播间标题，随控制条一起出现
            .when(full, |v| {
                v.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .px(px(SPACE_4))
                        .pt(px(SPACE_3))
                        .pb(px(theme::SPACE_8))
                        .bg(linear_gradient(180., linear_color_stop(c(0x0000008C), 0.), linear_color_stop(c(0x00000000), 1.)))
                        .t_title3()
                        .text_color(c(media.on_media))
                        .truncate()
                        .opacity(if self.controls_shown(playing) { 1. } else { 0. })
                        .child(d.title.clone()),
                )
            })
            .children((self.show_stats && room.player.is_some()).then(|| self.render_stats()))
            .children((!offline).then(|| self.render_controls(room, full, cx)))
            .children(self.menu.filter(|_| !offline).map(|m| self.render_menu(room, m, cx)));
        if full {
            return div().relative().size_full().flex().flex_col().child(video).children(self.render_banner(cx).map(|b| div().absolute().top_0().left_0().right_0().child(b))).into_any_element();
        }

        let followed = self.store.is_followed(site.id(), &d.room_id);
        let follow = if followed { button("follow", Btn::Normal, Some("check"), "已关注") } else { button("follow", Btn::Primary, Some("plus"), "关注") };
        let info = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(SPACE_3))
            .px(px(SPACE_4))
            .py(px(14.))
            .border_t_1()
            .border_color(border())
            .child(avatar(&d.user_avatar, &d.user_name, 56., d.live))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(SPACE_1))
                    .child(div().t_title3().truncate().child(d.title.clone()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(SPACE_2))
                            .t_callout()
                            .text_color(muted())
                            .child(div().min_w(px(0.)).truncate().text_color(text()).font_weight(FontWeight::SEMIBOLD).child(d.user_name.clone()))
                            .child(platform_tag(site.id(), site.name()))
                            .child(if d.live { div().flex().items_center().gap(px(SPACE_1)).child(icon("users", 12., muted())).child(fmt_count(room.online)) } else { status(false, "未开播") })
                            .children(d.live.then(|| fmt_live_for(d.start_time, now_secs() as i64)).flatten()),
                    ),
            )
            .children(room.qualities.get(room.quality_idx).map(|q| badge(Badge::Quality, None, q.name.clone())))
            .child(icon_btn("copy-link", "copy", false).tooltip(tip("复制链接", Some("⇧⌘C"))).on_click(cx.listener(|this, _, _, cx| this.copy_room_link(cx))))
            .child(icon_btn("room-more", "ellipsis", false).tooltip(tip("更多", None)).on_click(cx.listener(|this, ev: &gpui::ClickEvent, _, cx| {
                let pos = ev.position();
                let m = this.room_more_menu();
                this.open_ctx(pos, m, false, cx)
            })))
            .child(follow.tooltip(tip(if followed { "取消关注" } else { "关注主播" }, Some("⌘D"))).on_click(cx.listener(|this, _, _, cx| this.toggle_follow(cx))));

        let toolbar = self.render_toolbar(d.user_name.clone().into(), Some(format!("{} · 直播间", site.name()).into()), vec![], true, cx);
        let chat = AnyView::from(room.chat.clone()).cached(StyleRefinement::default().w(px(CHAT_WIDTH)).h_full().flex_none());
        let page = div()
            .size_full()
            .flex()
            .flex_col()
            .child(toolbar)
            .children(self.render_banner(cx).map(|b| b.mb(px(SPACE_3))))
            .child(div().flex_1().min_h(px(0.)).flex().child(div().flex_1().min_w(px(0.)).h_full().flex().flex_col().child(video).child(info)).children(self.show_chat.then_some(chat)));
        fade_in(page, ("room", room.id as usize)).into_any_element()
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
impl EntityInputHandler for Livo {
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
        let x = 36. + self.keyword[..r.start].chars().map(|c| if c as u32 >= 0x2E80 { 13. } else { 7. }).sum::<f32>();
        Some(gpui::Bounds::new(point(element.left() + px(x), element.top()), size(px(2.), element.size.height)))
    }

    fn character_index_for_point(&mut self, _: gpui::Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.keyword.encode_utf16().count())
    }
}

/// 画中画：置顶的小窗只画视频（同一个 Video 视图），双击回到主窗口
struct Pip {
    video: Entity<Video>,
    livo: gpui::WeakEntity<Livo>,
}

impl Render for Pip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let livo = self.livo.clone();
        div().id("pip").size_full().bg(rgb(0x000000)).child(self.video.clone()).on_click(move |ev, _, cx| {
            if ev.click_count() >= 2 {
                livo.update(cx, |this, cx| this.toggle_pip(cx)).ok();
            }
        })
    }
}

impl Livo {
    /// 画中画（⌥⌘P）：视频移到屏幕右下角的置顶小窗，主窗口的画面位置留一句提示；再按一次或双击小窗回来。
    /// shortcut: 离开直播间时画中画一起关掉，要做「主窗口离开直播间、小窗继续播」得把播放器从 Room 里拆出来
    fn toggle_pip(&mut self, cx: &mut Context<Self>) {
        if let Some(h) = self.pip.take() {
            h.update(cx, |_, window, _| window.remove_window()).ok();
            return cx.notify();
        }
        if self.room.as_ref().is_none_or(|r| r.urls.is_empty()) {
            return;
        }
        let (w, h) = (480., 270.);
        let screen = cx.primary_display().map(|d| d.bounds()).unwrap_or(Bounds { origin: point(px(0.), px(0.)), size: size(px(1440.), px(900.)) });
        let origin = screen.origin + point(screen.size.width - px(w + 24.), screen.size.height - px(h + 24.));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(w), px(h)) })),
            titlebar: Some(TitlebarOptions { title: Some("画中画".into()), appears_transparent: true, traffic_light_position: Some(point(px(10.), px(10.))) }),
            kind: gpui::WindowKind::PopUp,
            focus: false,
            window_min_size: Some(size(px(240.), px(135.))),
            ..Default::default()
        };
        let (video, livo) = (self.video.clone(), cx.entity().downgrade());
        match cx.open_window(options, |_, cx| {
            cx.new(|cx| {
                // 小窗被关掉（红灯）时主窗口把画面要回来
                cx.on_release(|this: &mut Pip, cx| {
                    this.livo.update(cx, |l, cx| {
                        l.pip = None;
                        cx.notify()
                    })
                    .ok();
                })
                .detach();
                Pip { video, livo }
            })
        }) {
            Ok(h) => self.pip = Some(h),
            Err(e) => self.error = Some(format!("打不开画中画：{e}")),
        }
        cx.notify();
    }
}

/// 主窗口；关掉它就退出（设置、画中画窗口关掉不退出）
static MAIN_WINDOW: OnceLock<gpui::AnyWindowHandle> = OnceLock::new();

fn main() {
    Application::new().with_assets(icons::Icons).with_http_client(Arc::new(img_http::ImageClient::new())).run(|cx: &mut App| {
        menus::init(cx);
        // 单窗口应用：关掉主窗口就退出（同“系统设置”），否则程序留在程序坞里却没有窗口
        cx.on_window_closed(|cx| {
            if MAIN_WINDOW.get().is_none_or(|main| !cx.windows().contains(main)) {
                cx.quit()
            }
        })
        .detach();
        // 记住上次的尺寸与位置；第一次启动居中
        let bounds = match Store::load().window {
            Some([x, y, w, h]) if w >= theme::WINDOW_MIN_WIDTH && h >= theme::WINDOW_MIN_HEIGHT => Bounds { origin: point(px(x), px(y)), size: size(px(w), px(h)) },
            _ => Bounds::centered(None, size(px(1280.), px(800.)), cx),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(theme::WINDOW_MIN_WIDTH), px(theme::WINDOW_MIN_HEIGHT))),
            // 标题栏与 52pt 工具栏合一，红绿灯在侧栏顶部垂直居中
            titlebar: Some(TitlebarOptions { title: Some("Livo".into()), appears_transparent: true, traffic_light_position: Some(point(px(20.), px(19.))) }),
            ..Default::default()
        };
        let main = cx
            .open_window(options, |window, cx| {
                let view = cx.new(|cx| {
                    // 外观设置“跟随系统”：系统切换深浅色时重绘
                    cx.observe_window_appearance(window, |this: &mut Livo, _, cx| this.refresh_theme(cx)).detach();
                    Livo::new(cx)
                });
                window.focus(&view.read(cx).focus);
                view
            })
            .expect("open window");
        MAIN_WINDOW.set(main.into()).ok();
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stall_detection() {
        let t0 = Instant::now();
        let at = |secs| t0 + Duration::from_secs(secs);
        assert!(is_stalled(true, t0, Some(t0), t0), "线路都断了");
        assert!(!is_stalled(false, t0, Some(at(5)), at(14)), "9 秒前还有画面");
        assert!(is_stalled(false, t0, Some(at(5)), at(16)), "11 秒没有新画面");
        assert!(!is_stalled(false, t0, None, at(29)), "首次缓冲给 30 秒");
        assert!(is_stalled(false, t0, None, at(31)), "30 秒还没出画面");
    }

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
    fn live_duration_reads_naturally() {
        assert_eq!(fmt_live_for(0, 100), None);
        assert_eq!(fmt_live_for(100, 130).as_deref(), Some("刚开播"));
        assert_eq!(fmt_live_for(1, 1 + 14 * 60).as_deref(), Some("开播 14 分钟"));
        assert_eq!(fmt_live_for(1, 1 + 2 * 3600).as_deref(), Some("开播 2 小时"));
        assert_eq!(fmt_live_for(1, 1 + 2 * 3600 + 14 * 60).as_deref(), Some("开播 2 小时 14 分"));
    }

    #[test]
    fn behind_is_timecode() {
        assert_eq!(fmt_behind(42.4), "-00:42");
        assert_eq!(fmt_behind(125.), "-02:05");
    }
}
