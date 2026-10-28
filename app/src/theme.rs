//! LIVO 设计系统主题常量：由设计系统的 tokens.json 生成，不要手改；改色请改设计系统后重新导出。
//! 颜色一律是 0xRRGGBBAA，直接传给 `gpui::rgba(..)`；不透明色的 AA 为 FF。

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub livo_violet: u32,
    pub livo_indigo: u32,
    pub livo_cyan: u32,
    pub livo_ink: u32,
    pub livo_night: u32,
    pub livo_night_edge: u32,
    pub livo_mist: u32,
    pub livo_mist_edge: u32,
    pub livo_shadow: u32,
    pub violet_300: u32,
    pub violet_600: u32,
    pub cyan_700: u32,
    pub bg: u32,
    pub sidebar: u32,
    pub surface: u32,
    pub surface_raised: u32,
    pub hover: u32,
    pub border: u32,
    pub border_strong: u32,
    pub hairline: u32,
    pub text: u32,
    pub text_muted: u32,
    pub text_subtle: u32,
    pub accent: u32,
    pub accent_fill: u32,
    pub accent_fill_hover: u32,
    pub on_accent: u32,
    pub accent_soft: u32,
    pub live: u32,
    pub live_fill: u32,
    pub on_live: u32,
    pub danger: u32,
    pub danger_fill: u32,
    pub danger_soft: u32,
    pub warning: u32,
    pub warning_soft: u32,
    pub success: u32,
    pub platform_bilibili: u32,
    pub platform_bilibili_text: u32,
    pub platform_bilibili_soft: u32,
    pub platform_douyu: u32,
    pub platform_douyu_text: u32,
    pub platform_douyu_soft: u32,
    pub platform_huya: u32,
    pub platform_huya_text: u32,
    pub platform_huya_soft: u32,
    pub scrim: u32,
    pub on_media: u32,
    pub danmaku_stroke: u32,
}

pub const DARK: Palette = Palette {
    livo_violet: 0x6E58F5FF,
    livo_indigo: 0x6B7CF0FF,
    livo_cyan: 0x4BD3CFFF,
    livo_ink: 0x171A23FF,
    livo_night: 0x1F2234FF,
    livo_night_edge: 0x353A54FF,
    livo_mist: 0xF8F9FDFF,
    livo_mist_edge: 0xE2E5F0FF,
    livo_shadow: 0x242B55FF,
    violet_300: 0xA99BFFFF,
    violet_600: 0x5A43E6FF,
    cyan_700: 0x0A7470FF,
    bg: 0x11131CFF,
    sidebar: 0x161925FF,
    surface: 0x1B1E2CFF,
    surface_raised: 0x1F2234FF,
    hover: 0x262A3DFF,
    border: 0x2A2E42FF,
    border_strong: 0x666E92FF,
    hairline: 0xFFFFFF14,
    text: 0xEEF0F8FF,
    text_muted: 0x9AA0B8FF,
    text_subtle: 0x868EAAFF,
    accent: 0xA99BFFFF,
    accent_fill: 0x6E58F5FF,
    accent_fill_hover: 0x5A43E6FF,
    on_accent: 0xFFFFFFFF,
    accent_soft: 0x6E58F52E,
    live: 0x4BD3CFFF,
    live_fill: 0x4BD3CFFF,
    on_live: 0x171A23FF,
    danger: 0xFF6B81FF,
    danger_fill: 0xD12C4BFF,
    danger_soft: 0xFF6B8124,
    warning: 0xF5B544FF,
    warning_soft: 0xF5B54424,
    success: 0x3DD68CFF,
    platform_bilibili: 0xEC79A9FF,
    platform_bilibili_text: 0xFB9DC2FF,
    platform_bilibili_soft: 0xEC79A929,
    platform_douyu: 0xF0834EFF,
    platform_douyu_text: 0xFEA47CFF,
    platform_douyu_soft: 0xF0834E29,
    platform_huya: 0xF2B036FF,
    platform_huya_text: 0xE8B45EFF,
    platform_huya_soft: 0xF2B03629,
    scrim: 0x00000099,
    on_media: 0xFFFFFFFF,
    danmaku_stroke: 0x000000B2,
};

pub const LIGHT: Palette = Palette {
    livo_violet: 0x6E58F5FF,
    livo_indigo: 0x6B7CF0FF,
    livo_cyan: 0x4BD3CFFF,
    livo_ink: 0x171A23FF,
    livo_night: 0x1F2234FF,
    livo_night_edge: 0x353A54FF,
    livo_mist: 0xF8F9FDFF,
    livo_mist_edge: 0xE2E5F0FF,
    livo_shadow: 0x242B55FF,
    violet_300: 0xA99BFFFF,
    violet_600: 0x5A43E6FF,
    cyan_700: 0x0A7470FF,
    bg: 0xF8F9FDFF,
    sidebar: 0xF1F3F9FF,
    surface: 0xFFFFFFFF,
    surface_raised: 0xFFFFFFFF,
    hover: 0xECEEF6FF,
    border: 0xE2E5F0FF,
    border_strong: 0x848AA2FF,
    hairline: 0x171A231A,
    text: 0x171A23FF,
    text_muted: 0x4E556CFF,
    text_subtle: 0x626880FF,
    accent: 0x5A43E6FF,
    accent_fill: 0x6E58F5FF,
    accent_fill_hover: 0x5A43E6FF,
    on_accent: 0xFFFFFFFF,
    accent_soft: 0x6E58F51A,
    live: 0x0A7470FF,
    live_fill: 0x4BD3CFFF,
    on_live: 0x171A23FF,
    danger: 0xBE2442FF,
    danger_fill: 0xD12C4BFF,
    danger_soft: 0xD12C4B14,
    warning: 0x9A6200FF,
    warning_soft: 0x9A620014,
    success: 0x137A4AFF,
    platform_bilibili: 0xEC79A9FF,
    platform_bilibili_text: 0x9A3E67FF,
    platform_bilibili_soft: 0xEC79A924,
    platform_douyu: 0xF0834EFF,
    platform_douyu_text: 0x9D4616FF,
    platform_douyu_soft: 0xF0834E24,
    platform_huya: 0xF2B036FF,
    platform_huya_text: 0x8B5700FF,
    platform_huya_soft: 0xF2B03629,
    scrim: 0x00000099,
    on_media: 0xFFFFFFFF,
    danmaku_stroke: 0x000000B2,
};

// spacing（逻辑像素）
pub const SPACE_HALF: f32 = 2.0;
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_6: f32 = 24.0;
pub const SPACE_8: f32 = 32.0;
pub const SPACE_12: f32 = 48.0;

// radius（逻辑像素）
pub const RADIUS_XS: f32 = 4.0;
pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS_MD: f32 = 10.0;
pub const RADIUS_LG: f32 = 14.0;
pub const RADIUS_FULL: f32 = 999.0;

// size（逻辑像素）
pub const CONTROL_SM: f32 = 24.0;
pub const CONTROL_MD: f32 = 28.0;
pub const CONTROL_LG: f32 = 32.0;
pub const SIDEBAR_WIDTH: f32 = 220.0;
pub const CHAT_WIDTH: f32 = 320.0;
pub const CARD_MIN: f32 = 220.0;
pub const TOOLBAR_HEIGHT: f32 = 52.0;
pub const ROW_HEIGHT: f32 = 28.0;
pub const WINDOW_MIN_WIDTH: f32 = 960.0;
pub const WINDOW_MIN_HEIGHT: f32 = 600.0;

// 字体：界面用系统字体（SF Pro + 苹方），GPUI 里用 ".SystemUIFont"；品牌排版（官网、物料）用 Outfit + Noto Sans SC，应用内不打包。
pub const FONT_UI: &str = ".SystemUIFont";
pub const FONT_MONO: &str = "SF Mono";

// 字号（px）与行高：对应设计系统的界面文字样式
pub const TEXT_LARGE_TITLE: (f32, f32) = (26.0, 32.0); // weight 700
pub const TEXT_TITLE_1: (f32, f32) = (22.0, 28.0); // weight 600
pub const TEXT_TITLE_2: (f32, f32) = (17.0, 22.0); // weight 600
pub const TEXT_TITLE_3: (f32, f32) = (15.0, 20.0); // weight 600
pub const TEXT_HEADLINE: (f32, f32) = (13.0, 18.0); // weight 600
pub const TEXT_BODY: (f32, f32) = (13.0, 20.0); // weight 400
pub const TEXT_CALLOUT: (f32, f32) = (12.0, 18.0); // weight 400
pub const TEXT_CAPTION: (f32, f32) = (11.0, 14.0); // weight 500
pub const TEXT_DANMAKU: (f32, f32) = (20.0, 26.0); // weight 600
pub const TEXT_TIMECODE: (f32, f32) = (12.0, 16.0); // weight 500
