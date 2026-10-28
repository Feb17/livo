//! macOS 菜单栏（按 Apple 的标准菜单顺序：应用、编辑、显示、播放、窗口、帮助）。
//! 菜单项都是 GPUI action：退出、隐藏、帮助这类应用级的在这里全局处理，其余由根视图处理；
//! 直播间相关的只在直播间里注册，不在直播间时菜单项自动变灰。
//! 播放页的单键快捷键可以自定义（store::KEY_ACTIONS），所以“播放”菜单只放固定的 ⌘ 快捷键
use crate::about::REPO;
use gpui::{actions, App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType};

actions!(
    livo,
    [
        Quit, Hide, HideOthers, ShowAll, About, CheckUpdate, Settings, Help, ReportIssue,
        EditCut, EditCopy, EditPaste, FocusSearch,
        ShowRecommend, ShowCategory, ShowFollow, ShowHistory, ToggleFullScreen,
        TogglePause, ToggleMute, VolumeUp, VolumeDown, SeekBack, SeekForward, ToggleDanmaku, ToggleChat, ToggleStats, Refresh, ToggleFollow, CopyLink, CloseRoom,
        Minimize, Zoom, CloseWindow,
    ]
);

pub fn init(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Help, cx| cx.open_url(&format!("{REPO}#readme")));
    cx.on_action(|_: &ReportIssue, cx| cx.open_url(&format!("{REPO}/issues")));
    // 菜单上显示的快捷键也从这里来
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-,", Settings, None),
        KeyBinding::new("cmd-x", EditCut, None),
        KeyBinding::new("cmd-c", EditCopy, None),
        KeyBinding::new("cmd-v", EditPaste, None),
        KeyBinding::new("cmd-f", FocusSearch, None),
        KeyBinding::new("cmd-1", ShowRecommend, None),
        KeyBinding::new("cmd-2", ShowCategory, None),
        KeyBinding::new("cmd-3", ShowFollow, None),
        KeyBinding::new("cmd-4", ShowHistory, None),
        // 菜单项由 AppKit 自己加在窗口菜单里（“进入/退出全屏幕”），这里只保证快捷键
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        KeyBinding::new("cmd-r", Refresh, None),
        KeyBinding::new("cmd-d", ToggleFollow, None),
        KeyBinding::new("shift-cmd-c", CopyLink, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
    ]);
    cx.set_menus(vec![
        Menu {
            name: "Livo".into(),
            items: vec![
                MenuItem::action("关于 Livo", About),
                MenuItem::action("检查更新…", CheckUpdate),
                MenuItem::separator(),
                MenuItem::action("设置…", Settings),
                MenuItem::separator(),
                MenuItem::os_submenu("服务", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("隐藏 Livo", Hide),
                MenuItem::action("隐藏其他", HideOthers),
                MenuItem::action("全部显示", ShowAll),
                MenuItem::separator(),
                MenuItem::action("退出 Livo", Quit),
            ],
        },
        // 唯一的文本框是搜索关键词，没有选区，剪切/拷贝针对整个关键词；⌘F 让工具栏的搜索框进入输入状态
        Menu {
            name: "编辑".into(),
            items: vec![
                MenuItem::os_action("剪切", EditCut, OsAction::Cut),
                MenuItem::os_action("拷贝", EditCopy, OsAction::Copy),
                MenuItem::os_action("粘贴", EditPaste, OsAction::Paste),
                MenuItem::separator(),
                MenuItem::action("搜索", FocusSearch),
            ],
        },
        Menu {
            name: "显示".into(),
            items: vec![
                MenuItem::action("推荐", ShowRecommend),
                MenuItem::action("分区", ShowCategory),
                MenuItem::action("我的关注", ShowFollow),
                MenuItem::action("观看记录", ShowHistory),
                MenuItem::separator(),
                MenuItem::action("刷新", Refresh),
            ],
        },
        Menu {
            name: "播放".into(),
            items: vec![
                MenuItem::action("暂停/继续", TogglePause),
                MenuItem::action("后退 10 秒", SeekBack),
                MenuItem::action("前进 10 秒", SeekForward),
                MenuItem::separator(),
                MenuItem::action("静音/取消静音", ToggleMute),
                MenuItem::action("调高音量", VolumeUp),
                MenuItem::action("调低音量", VolumeDown),
                MenuItem::separator(),
                MenuItem::action("显示/隐藏弹幕", ToggleDanmaku),
                MenuItem::action("收起/展开聊天区", ToggleChat),
                MenuItem::action("显示/隐藏调试信息", ToggleStats),
                MenuItem::separator(),
                MenuItem::action("关注/取消关注", ToggleFollow),
                MenuItem::action("复制直播间链接", CopyLink),
                MenuItem::action("离开直播间", CloseRoom),
            ],
        },
        Menu { name: "窗口".into(), items: vec![MenuItem::action("最小化", Minimize), MenuItem::action("缩放", Zoom), MenuItem::separator(), MenuItem::action("关闭", CloseWindow)] },
        Menu { name: "帮助".into(), items: vec![MenuItem::action("Livo 帮助", Help), MenuItem::action("报告问题…", ReportIssue)] },
    ]);
    #[cfg(target_os = "macos")]
    register_system_menus();
}

/// GPUI 只认英文标题 "Window" 的窗口菜单，也不设帮助菜单；这里把最后两个菜单交给 AppKit，
/// 系统才会补上窗口列表、“前置全部窗口”、平铺和帮助搜索框
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)] // objc 0.2 的 msg_send! 里有 cfg(feature = "cargo-clippy")
fn register_system_menus() {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let bar: *mut Object = msg_send![app, mainMenu];
        let n: isize = msg_send![bar, numberOfItems];
        let submenu = |i: isize| -> *mut Object {
            let item: *mut Object = msg_send![bar, itemAtIndex: i];
            msg_send![item, submenu]
        };
        let _: () = msg_send![app, setWindowsMenu: submenu(n - 2)];
        let _: () = msg_send![app, setHelpMenu: submenu(n - 1)];
    }
}
