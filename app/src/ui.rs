//! 设计系统（LIVO 设计系统）在 GPUI 里的落地：主题色板、字号、阴影和通用控件。
//! 颜色一律来自 theme.rs（设计系统导出的 tokens），这里只按当前外观取值，不再手写色值。
use crate::icons::icon;
use crate::theme::{self, Palette};
use gpui::{
    div, img, point, prelude::*, px, rgba, Animation, AnimationExt, AnyElement, AnyView, App, BoxShadow, Context, Div, FontWeight, Hsla, ObjectFit, Rgba, SharedString, Stateful, Styled, Svg, Window,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// 浅色/深色由根视图每帧按设置（或系统外观）切换
pub static LIGHT: AtomicBool = AtomicBool::new(false);
/// 系统「减弱动态效果」：根视图每帧读一次，开启时去掉位移和呼吸，只留透明度或直接显示
pub static REDUCE_MOTION: AtomicBool = AtomicBool::new(false);

pub fn reduce_motion() -> bool {
    REDUCE_MOTION.load(Ordering::Relaxed)
}

/// 读系统辅助功能里的「减弱动态效果」
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)] // objc 0.2 的 msg_send! 里有 cfg(feature = "cargo-clippy")
pub fn system_reduce_motion() -> bool {
    use objc::runtime::{Object, BOOL, YES};
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let on: BOOL = msg_send![ws, accessibilityDisplayShouldReduceMotion];
        on == YES
    }
}

/// 键盘焦点环：2px accent 实线环，外偏移 2px（先垫一圈底色当偏移）。只在键盘操作时出现，鼠标点完焦点会还给根视图
pub fn focus_ring() -> Vec<BoxShadow> {
    let ring = |color: Rgba, spread: f32| BoxShadow { color: color.into(), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(spread) };
    vec![ring(bg(), 2.), ring(accent(), 4.)]
}

/// 可用 Tab 键到达的控件：加入 tab 顺序，聚焦时画焦点环；回车 / 空格触发 on_click（GPUI 自带）
pub trait Focusable: StatefulInteractiveElement + Sized {
    fn focusable_ring(self) -> Self {
        self.tab_index(0).focus(|s| s.shadow(focus_ring()))
    }
}
impl<E: StatefulInteractiveElement + Sized> Focusable for E {}

/// 当前外观的色板；播放器区域不跟随主题，直接用 theme::DARK
pub fn pal() -> &'static Palette {
    if LIGHT.load(Ordering::Relaxed) {
        &theme::LIGHT
    } else {
        &theme::DARK
    }
}

/// 0xRRGGBBAA → 颜色
pub fn c(v: u32) -> Rgba {
    rgba(v)
}

pub fn bg() -> Rgba {
    c(pal().bg)
}
pub fn sidebar() -> Rgba {
    c(pal().sidebar)
}
pub fn surface() -> Rgba {
    c(pal().surface)
}
pub fn raised() -> Rgba {
    c(pal().surface_raised)
}
pub fn hover() -> Rgba {
    c(pal().hover)
}
pub fn border() -> Rgba {
    c(pal().border)
}
pub fn border_strong() -> Rgba {
    c(pal().border_strong)
}
pub fn text() -> Rgba {
    c(pal().text)
}
pub fn muted() -> Rgba {
    c(pal().text_muted)
}
pub fn subtle() -> Rgba {
    c(pal().text_subtle)
}
/// 紫色文字、选中态
pub fn accent() -> Rgba {
    c(pal().accent)
}
/// 紫色填充：主按钮、开关、滑块已选段
pub fn accent_fill() -> Rgba {
    c(pal().accent_fill)
}
pub fn accent_soft() -> Rgba {
    c(pal().accent_soft)
}
/// 错误文字与冲突描边
pub fn danger() -> Rgba {
    c(pal().danger)
}
/// 青色 = 直播中
pub fn live() -> Rgba {
    c(pal().live)
}

/// 平台色：(圆点, 标签文字, 标签底色)，只做小面积标记
pub fn platform(site: &str) -> (Rgba, Rgba, Rgba) {
    let p = pal();
    let (dot, text, soft) = match site {
        "bilibili" => (p.platform_bilibili, p.platform_bilibili_text, p.platform_bilibili_soft),
        "douyu" => (p.platform_douyu, p.platform_douyu_text, p.platform_douyu_soft),
        "huya" => (p.platform_huya, p.platform_huya_text, p.platform_huya_soft),
        _ => (p.border_strong, p.text_muted, p.hover),
    };
    (c(dot), c(text), c(soft))
}

fn shadow(color: u32, y: f32, blur: f32) -> BoxShadow {
    BoxShadow { color: c(color).into(), offset: point(px(0.), px(y)), blur_radius: px(blur), spread_radius: px(0.) }
}

/// shadow-card：悬停卡片、分段控件的选中块
pub fn shadow_card() -> Vec<BoxShadow> {
    if LIGHT.load(Ordering::Relaxed) {
        vec![shadow(0x242B5514, 1., 2.), shadow(0x242B550F, 4., 12.)]
    } else {
        vec![shadow(0x00000066, 1., 2.)]
    }
}

/// shadow-popover：菜单、弹出面板、Toast
pub fn shadow_popover(dark: bool) -> Vec<BoxShadow> {
    if dark || !LIGHT.load(Ordering::Relaxed) {
        vec![shadow(0x00000080, 12., 32.)]
    } else {
        vec![shadow(0x242B5529, 12., 32.)]
    }
}

/// 界面文字样式（macOS 字号）：字号/行高/字重成套使用
pub trait Type: Styled + Sized {
    fn t_title2(self) -> Self {
        self.text_size(px(17.)).line_height(px(22.)).font_weight(FontWeight::SEMIBOLD)
    }
    fn t_title3(self) -> Self {
        self.text_size(px(15.)).line_height(px(20.)).font_weight(FontWeight::SEMIBOLD)
    }
    fn t_headline(self) -> Self {
        self.text_size(px(13.)).line_height(px(18.)).font_weight(FontWeight::SEMIBOLD)
    }
    fn t_body(self) -> Self {
        self.text_size(px(13.)).line_height(px(20.))
    }
    fn t_callout(self) -> Self {
        self.text_size(px(12.)).line_height(px(18.))
    }
    fn t_caption(self) -> Self {
        self.text_size(px(11.)).line_height(px(14.)).font_weight(FontWeight::MEDIUM)
    }
    fn t_timecode(self) -> Self {
        self.font_family(MONO).text_size(px(12.)).line_height(px(16.)).font_weight(FontWeight::MEDIUM)
    }
}
impl<E: Styled> Type for E {}

/// 数字信息（码率、时移）用等宽字；SF Mono 不一定能按名字取到，用系统自带的 Menlo
pub const MONO: &str = "Menlo";

/// 图标按钮：28px 方形、20px 图标，悬停 hover 底；切换类打开时图标变 accent
pub fn icon_btn(id: impl Into<SharedString>, name: &'static str, pressed: bool) -> Stateful<Div> {
    let id = id.into();
    // group 名带上 id：同一行多个图标按钮各自悬停
    let group = SharedString::from(format!("icon-btn-{id}"));
    let color = if pressed { accent() } else { muted() };
    div()
        .id(id)
        .group(group.clone())
        .size(px(theme::CONTROL_MD))
        .flex_none()
        .rounded(px(theme::RADIUS_SM))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(hover()))
        .focusable_ring()
        .child(icon(name, 20., color).when(!pressed, |i| i.group_hover(group, |s| s.text_color(text()))))
}

/// 禁用的图标按钮：text-subtle、60% 不透明，不响应悬停（如没有可前进的页面时的「前进」）
pub fn icon_btn_disabled(id: impl Into<SharedString>, name: &'static str) -> Stateful<Div> {
    div().id(id.into()).size(px(theme::CONTROL_MD)).flex_none().flex().items_center().justify_center().opacity(0.6).child(icon(name, 20., subtle()))
}

/// 播放器上的图标按钮：32px 点击区，图标 on-media，打开时 violet-300
pub fn media_btn(id: impl Into<SharedString>, name: &'static str, pressed: bool) -> Stateful<Div> {
    let d = &theme::DARK;
    div()
        .id(id.into())
        .size(px(32.))
        .flex_none()
        .rounded(px(theme::RADIUS_SM))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(c(0xFFFFFF24)))
        .child(icon(name, 20., c(if pressed { d.violet_300 } else { d.on_media })))
}

/// 播放器上的文字按钮 lv-popup-media：26px 高、白色 30% 描边，不跟随主题
pub fn media_btn_text(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    let d = &theme::DARK;
    div()
        .id(id.into())
        .h(px(26.))
        .flex_none()
        .px(px(theme::SPACE_2))
        .rounded(px(theme::RADIUS_SM))
        .border_1()
        .border_color(c(0xFFFFFF4D))
        .flex()
        .items_center()
        .gap(px(theme::SPACE_1))
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(d.on_media))
        .cursor_pointer()
        .hover(|s| s.bg(c(0xFFFFFF24)))
        .focusable_ring()
        .child(label.into())
}

#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    /// 次操作（默认）
    Normal,
    /// 主操作：一屏只有一个
    Primary,
    /// 取消、工具栏文字按钮
    Ghost,
}

/// 按钮：28px 高，文字动词开头，可带 16px 前置图标
pub fn button(id: impl Into<SharedString>, kind: Btn, leading: Option<&'static str>, label: impl Into<SharedString>) -> Stateful<Div> {
    let p = pal();
    let (bg_, border_, fg, hover_bg) = match kind {
        Btn::Normal => (p.surface, p.border_strong, p.text, p.hover),
        Btn::Primary => (p.accent_fill, p.accent_fill, p.on_accent, p.accent_fill_hover),
        Btn::Ghost => (0, 0, p.text_muted, p.hover),
    };
    let hover_fg = if kind == Btn::Ghost { p.text } else { fg };
    div()
        .id(id.into())
        .h(px(theme::CONTROL_MD))
        .flex_none()
        .px(px(theme::SPACE_3))
        .rounded(px(theme::RADIUS_SM))
        .border_1()
        .border_color(c(border_))
        .bg(c(bg_))
        .flex()
        .items_center()
        .gap(px(theme::SPACE_2))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(fg))
        .cursor_pointer()
        .hover(move |s| s.bg(c(hover_bg)).text_color(c(hover_fg)).border_color(c(if kind == Btn::Normal || kind == Btn::Ghost { border_ } else { hover_bg })))
        .focusable_ring()
        .children(leading.map(|n| icon(n, 16., c(fg))))
        .child(label.into())
}

/// 大号按钮 lv-btn-lg：32px 高、左右 space-4，用于搜索栏旁和空状态主操作
pub fn large(b: Stateful<Div>) -> Stateful<Div> {
    b.h(px(theme::CONTROL_LG)).px(px(theme::SPACE_4))
}

/// 弹出按钮 lv-popup：显示当前值 + chevron-down，点击后在下方弹出单选菜单
pub fn popup_button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .h(px(theme::CONTROL_MD))
        .flex_none()
        .pl(px(theme::SPACE_3))
        .pr(px(theme::SPACE_2))
        .rounded(px(theme::RADIUS_SM))
        .border_1()
        .border_color(border_strong())
        .bg(surface())
        .flex()
        .items_center()
        .gap(px(theme::SPACE_2))
        .t_body()
        .text_color(text())
        .cursor_pointer()
        .hover(|s| s.bg(hover()))
        .focusable_ring()
        .child(label.into())
        .child(icon("chevron-down", 14., muted()))
}

/// 菜单项：普通 / 单选（打勾）/ 破坏性，可带 16px 图标和右侧快捷键
pub enum MenuEntry<T> {
    Item { label: SharedString, icon: Option<&'static str>, key: Option<&'static str>, checked: Option<bool>, danger: bool, action: T },
    Sep,
}

impl<T> MenuEntry<T> {
    pub fn item(label: impl Into<SharedString>, icon: Option<&'static str>, action: T) -> Self {
        MenuEntry::Item { label: label.into(), icon, key: None, checked: None, danger: false, action }
    }
    pub fn radio(label: impl Into<SharedString>, checked: bool, action: T) -> Self {
        MenuEntry::Item { label: label.into(), icon: None, key: None, checked: Some(checked), danger: false, action }
    }
    pub fn key(mut self, k: &'static str) -> Self {
        if let MenuEntry::Item { key, .. } = &mut self {
            *key = Some(k);
        }
        self
    }
    pub fn danger(mut self) -> Self {
        if let MenuEntry::Item { danger, .. } = &mut self {
            *danger = true;
        }
        self
    }
}

/// 菜单 lv-menu：surface-raised + shadow-popover，行高 24；高亮项 accent-fill 底 + on-accent 字（macOS 惯例），破坏性项高亮用 danger-fill。
/// `on_pick` 收到被点中的那一项的 action
pub fn menu<T: Clone + 'static>(entries: Vec<MenuEntry<T>>, on_pick: impl Fn(T, &mut Window, &mut App) + 'static) -> Div {
    let p = pal();
    let on_pick = std::rc::Rc::new(on_pick);
    let checkable = entries.iter().any(|e| matches!(e, MenuEntry::Item { checked: Some(_), .. }));
    div()
        .min_w(px(200.))
        .p(px(5.))
        .rounded(px(theme::RADIUS_MD))
        .bg(raised())
        .border_1()
        .border_color(border())
        .shadow(shadow_popover(false))
        .flex()
        .flex_col()
        .t_body()
        .text_color(text())
        .children(entries.into_iter().enumerate().map(move |(i, e)| match e {
            MenuEntry::Sep => div().mx(px(6.)).my(px(5.)).h(px(1.)).bg(border()).into_any_element(),
            MenuEntry::Item { label, icon: lead, key, checked, danger, action } => {
                let group = SharedString::from(format!("menu-item-{i}"));
                let (fg, hi) = if danger { (p.danger, p.danger_fill) } else { (p.text, p.accent_fill) };
                let on_pick = on_pick.clone();
                let tint = move |e: Svg| e.group_hover(group.clone(), |s| s.text_color(c(p.on_accent)));
                div()
                    .id(("menu-item", i))
                    .group(SharedString::from(format!("menu-item-{i}")))
                    .h(px(24.))
                    .pl(px(6.))
                    .pr(px(theme::SPACE_2))
                    .rounded(px(5.))
                    .flex()
                    .items_center()
                    .gap(px(theme::SPACE_2))
                    .text_color(c(fg))
                    .cursor_pointer()
                    .hover(move |s| s.bg(c(hi)).text_color(c(p.on_accent)))
                    .focusable_ring()
                    .on_click(move |_, window, cx| on_pick(action.clone(), window, cx))
                    .when(checkable, |d| d.child(tint(icon("check", 14., c(fg))).when(checked != Some(true), |i| i.invisible())))
                    .children(lead.map(|n| tint(icon(n, 16., c(fg)))))
                    .child(div().flex_1().child(label))
                    .children(key.map(|k| div().pl(px(theme::SPACE_4)).t_callout().text_color(subtle()).group_hover(SharedString::from(format!("menu-item-{i}")), |s| s.text_color(c(p.on_accent))).child(k)))
                    .into_any_element()
            }
        }))
}

/// 键帽：设置里快捷键的显示
pub fn keycap(label: impl Into<SharedString>) -> Div {
    div().min_w(px(20.)).h(px(20.)).px(px(5.)).flex().items_center().justify_center().rounded(px(theme::RADIUS_XS)).bg(hover()).border_1().border_color(border()).text_size(px(12.)).child(label.into())
}

/// 徽标底色：LIVE、清晰度、中性、压在封面上的人气
#[derive(Clone, Copy)]
pub enum Badge {
    Live,
    Quality,
    Neutral,
    Media,
}

pub fn badge(kind: Badge, leading: Option<&'static str>, label: impl Into<SharedString>) -> Div {
    let p = pal();
    let (bg_, fg) = match kind {
        Badge::Live => (p.live_fill, p.on_live),
        Badge::Quality => (p.accent_soft, p.accent),
        Badge::Neutral => (p.hover, p.text_muted),
        Badge::Media => (p.scrim, p.on_media),
    };
    div()
        .h(px(18.))
        .flex_none()
        .px(px(6.))
        .rounded(px(theme::RADIUS_XS))
        .flex()
        .items_center()
        .gap(px(theme::SPACE_1))
        .bg(c(bg_))
        .text_color(c(fg))
        .t_caption()
        .font_weight(FontWeight::SEMIBOLD)
        .children(leading.map(|n| icon(n, 12., c(fg))))
        .child(label.into())
}

/// 平台标签：平台圆点 + 平台名，永远带文字
pub fn platform_tag(site: &str, name: &'static str) -> Div {
    let (dot, fg, soft) = platform(site);
    div()
        .h(px(18.))
        .flex_none()
        .px(px(6.))
        .rounded(px(theme::RADIUS_XS))
        .flex()
        .items_center()
        .gap(px(5.))
        .bg(soft)
        .text_color(fg)
        .t_caption()
        .font_weight(FontWeight::SEMIBOLD)
        .child(div().size(px(6.)).rounded_full().bg(dot))
        .child(name)
}

/// 在播状态：6px 圆点 + 12px 文字
pub fn status(on: bool, label: &'static str) -> Div {
    let color = if on { live() } else { subtle() };
    div().flex().items_center().gap(px(6.)).t_callout().text_color(color).child(div().size(px(6.)).rounded_full().bg(color)).child(label)
}

/// 头像：圆形 + hairline；无图显示名字首字；在播加 2px 青色外环（与头像隔 2px，负外边距不占布局）
pub fn avatar(url: &str, name: &str, size: f32, live: bool) -> Div {
    let p = pal();
    let initial: SharedString = name.chars().next().map(|ch| ch.to_string()).unwrap_or_default().into();
    let font = (size * 0.38).round().max(10.);
    let fallback = move || -> AnyElement {
        let d = div().size_full().rounded_full().bg(hover()).flex().items_center().justify_center();
        if initial.is_empty() {
            d.child(icon("user", (size * 0.5).round(), muted())).into_any_element()
        } else {
            d.text_size(px(font)).font_weight(FontWeight::SEMIBOLD).text_color(muted()).child(initial.clone()).into_any_element()
        }
    };
    let image = if url.is_empty() {
        fallback()
    } else {
        img(SharedString::from(url.to_string())).size_full().rounded_full().object_fit(ObjectFit::Cover).with_fallback(fallback).into_any_element()
    };
    let face = div().relative().size(px(size)).flex_none().rounded_full().child(image).child(div().absolute().inset_0().rounded_full().border_1().border_color(c(p.hairline)));
    if live {
        div().flex_none().m(px(-4.)).p(px(2.)).rounded_full().border_2().border_color(c(p.live_fill)).child(face)
    } else {
        face
    }
}

/// 空状态：56px 圆形图标 + 一句现状 + 一句下一步，可选一个按钮
pub fn empty_state(name: &'static str, title: impl Into<SharedString>, desc: impl Into<SharedString>, action: Option<AnyElement>) -> Div {
    div()
        .w_full()
        .py(px(theme::SPACE_8))
        .px(px(theme::SPACE_6))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(theme::SPACE_2))
        .child(div().size(px(56.)).mb(px(theme::SPACE_1)).rounded_full().bg(hover()).flex().items_center().justify_center().child(icon(name, 24., muted())))
        .child(div().t_title3().text_center().child(title.into()))
        .child(div().max_w(px(280.)).mb(px(theme::SPACE_2)).t_callout().text_center().text_color(muted()).child(desc.into()))
        .children(action)
}

/// 骨架占位：hover 色块，透明度在 1 与 0.55 之间缓慢呼吸
pub fn skeleton(id: impl Into<gpui::ElementId>, w: f32, h: f32, radius: f32) -> AnyElement {
    let d = div().w(px(w)).h(px(h)).flex_none().rounded(px(radius)).bg(hover());
    // 「减弱动态效果」开启时静止
    if reduce_motion() {
        return d.into_any_element();
    }
    d.with_animation(id, Animation::new(Duration::from_millis(1400)).repeat(), |d, t| d.opacity(1. - 0.45 * (1. - (2. * t - 1.).abs()))).into_any_element()
}

/// 分组标题：11px text-subtle
pub fn group_label(label: impl Into<SharedString>) -> Div {
    div().px(px(theme::SPACE_1)).t_caption().font_weight(FontWeight::SEMIBOLD).text_color(subtle()).child(label.into())
}

/// 分区标题 lv-section-head：17/22 600 的标题 + 12px text-muted 的数量或说明，基线对齐，下方留 space-3
pub fn section_head(title: impl Into<SharedString>, note: Option<String>) -> Div {
    div()
        .flex()
        .items_baseline()
        .gap(px(theme::SPACE_2))
        .mb(px(theme::SPACE_3))
        .child(div().t_title2().child(title.into()))
        .children(note.map(|n| div().t_callout().text_color(muted()).child(n)))
}

/// 悬停 0.5 秒后出现的文字说明（动作名 + 可选快捷键）
pub struct Tooltip {
    label: SharedString,
    key: Option<SharedString>,
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(theme::SPACE_2))
            .py(px(theme::SPACE_1))
            .rounded(px(theme::RADIUS_SM))
            .bg(raised())
            .border_1()
            .border_color(border())
            .shadow(shadow_popover(false))
            .flex()
            .items_center()
            .gap(px(theme::SPACE_2))
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(text())
            .child(self.label.clone())
            .children(self.key.clone().map(kbd))
    }
}

/// 快捷键提示 lv-kbd：11px text-subtle，细描边（搜索框右侧、Tooltip 里）
pub fn kbd(label: impl Into<SharedString>) -> Div {
    div().flex_none().px(px(5.)).py(px(1.)).rounded(px(theme::RADIUS_XS)).border_1().border_color(border_strong()).text_size(px(11.)).line_height(px(14.)).text_color(subtle()).child(label.into())
}

/// 给图标按钮用：`.tooltip(tip("刷新", Some("⌘R")))`
pub fn tip(label: impl Into<SharedString>, key: Option<&str>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let (label, key): (SharedString, Option<SharedString>) = (label.into(), key.map(|k| SharedString::from(k.to_string())));
    move |_, cx| cx.new(|_| Tooltip { label: label.clone(), key: key.clone() }).into()
}

/// 两个颜色按 t 插值（开关轨道过渡用）
pub fn mix(a: Rgba, b: Rgba, t: f32) -> Hsla {
    Rgba { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t, a: a.a + (b.a - a.a) * t }.into()
}
