//! 设计系统（LIVO 设计系统）在 GPUI 里的落地：主题色板、字号、阴影和通用控件。
//! 颜色一律来自 theme.rs（设计系统导出的 tokens），这里只按当前外观取值，不再手写色值。
use crate::icons::icon;
use crate::theme::{self, Palette};
use gpui::{
    div, img, point, prelude::*, px, rgba, Animation, AnimationExt, AnyElement, AnyView, App, BoxShadow, Context, Div, FontWeight, Hsla, ObjectFit, Rgba, SharedString, Stateful, Styled, Window,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// 浅色/深色由根视图每帧按设置（或系统外观）切换
pub static LIGHT: AtomicBool = AtomicBool::new(false);

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
    let color = if pressed { accent() } else { muted() };
    div()
        .id(id.into())
        .group("icon-btn")
        .size(px(theme::CONTROL_MD))
        .flex_none()
        .rounded(px(theme::RADIUS_SM))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(hover()))
        .child(icon(name, 20., color).when(!pressed, |i| i.group_hover("icon-btn", |s| s.text_color(text()))))
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
        .hover(move |s| s.bg(c(hover_bg)).border_color(c(if kind == Btn::Normal || kind == Btn::Ghost { border_ } else { hover_bg })))
        .children(leading.map(|n| icon(n, 16., c(fg))))
        .child(label.into())
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
        .py(px(theme::SPACE_8 * 2.))
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
pub fn skeleton(id: impl Into<gpui::ElementId>, w: f32, h: f32, radius: f32) -> impl IntoElement {
    div().w(px(w)).h(px(h)).flex_none().rounded(px(radius)).bg(hover()).with_animation(id, Animation::new(Duration::from_millis(1400)).repeat(), |d, t| d.opacity(1. - 0.45 * (1. - (2. * t - 1.).abs())))
}

/// 分组标题：11px text-subtle
pub fn group_label(label: impl Into<SharedString>) -> Div {
    div().px(px(theme::SPACE_1)).t_caption().font_weight(FontWeight::SEMIBOLD).text_color(subtle()).child(label.into())
}

/// 列表上方的分组标题：标题 + 数量
pub fn section_head(title: &'static str, count: Option<usize>) -> Div {
    div().flex().items_center().gap(px(theme::SPACE_2)).child(div().t_headline().child(title)).children(count.map(|n| div().t_callout().text_color(subtle()).child(n.to_string())))
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
            .t_callout()
            .text_color(text())
            .child(self.label.clone())
            .children(self.key.clone().map(|k| div().text_color(subtle()).child(k)))
    }
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
