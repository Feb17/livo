//! 本地设置与关注列表，存成一个 JSON 文件（同原项目的设置项与 FollowUser）。
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 视频上滚动弹幕的样式；默认值同原项目
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DanmakuStyle {
    /// 字号（px）
    pub font: f32,
    /// 不透明度 0.1-1
    pub opacity: f32,
    /// 横穿画面的秒数，越小越快
    pub secs: f32,
    /// 占画面上方的比例 0.1-1
    pub area: f32,
    /// 进入直播间时是否显示滚动弹幕
    pub enabled: bool,
}

impl Default for DanmakuStyle {
    fn default() -> Self {
        Self { font: 16., opacity: 1., secs: 10., area: 0.8, enabled: true }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// 直播设置（同原项目“直播间设置”里桌面端有意义的部分）
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LiveSettings {
    /// 默认清晰度：0 最低，1 中等，2 最高，3 记住上次
    pub quality_level: u8,
    /// 记住上次：平台 id -> 清晰度名称
    pub quality_memory: BTreeMap<String, String>,
    pub hwdec: bool,
    /// 网络缓冲（MiB），时移可回看的时长也由它决定
    pub buffer_mb: u32,
    /// 画面尺寸：0 适应，1 填充，2 拉伸
    pub fit: u8,
    /// 把 http 播放地址换成 https
    pub https: bool,
    pub auto_fullscreen: bool,
}

impl Default for LiveSettings {
    fn default() -> Self {
        Self { quality_level: 1, quality_memory: BTreeMap::new(), hwdec: true, buffer_mb: 16, fit: 0, https: false, auto_fullscreen: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FollowSettings {
    pub auto_refresh: bool,
    /// 自动更新间隔（分钟）
    pub interval_min: u32,
    /// 同时查询的房间数
    pub threads: u32,
}

impl Default for FollowSettings {
    fn default() -> Self {
        Self { auto_refresh: true, interval_min: 10, threads: 4 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoExit {
    pub enabled: bool,
    pub minutes: u32,
}

impl Default for AutoExit {
    fn default() -> Self {
        Self { enabled: false, minutes: 60 }
    }
}

/// B站登录态
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BiliAccount {
    pub cookie: String,
    pub uid: i64,
    pub name: String,
}

/// 播放页快捷键可配置的动作及默认按键（按键名用 GPUI 的 keystroke.key）
pub const KEY_ACTIONS: [(&str, &str, &str); 11] = [
    ("fullscreen", "切换全屏", "f"),
    ("danmaku", "显示/隐藏弹幕", "d"),
    ("mute", "静音/取消静音", "m"),
    ("refresh", "刷新直播间", "r"),
    ("chat", "收起/展开聊天区", "c"),
    ("volume_up", "调高音量", "up"),
    ("volume_down", "调低音量", "down"),
    ("pause", "暂停/继续", "space"),
    ("seek_back", "后退 10 秒（时移）", "left"),
    ("seek_forward", "前进 10 秒（时移）", "right"),
    ("stats", "显示/隐藏调试信息", "i"),
];

fn default_keys() -> BTreeMap<String, String> {
    KEY_ACTIONS.iter().map(|(a, _, k)| (a.to_string(), k.to_string())).collect()
}

/// 主页（侧边栏）页面的默认顺序，第一个是启动页
pub const HOME_PAGES: [(&str, &str); 3] = [("recommend", "热门推荐"), ("follow", "我的关注"), ("category", "全部分类")];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Follow {
    /// Site::id()
    pub site: String,
    pub room_id: String,
    pub user_name: String,
    pub face: String,
    pub title: String,
    pub cover: String,
}

/// 观看记录（同原项目 History，id = 平台_房间号）
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub site: String,
    pub room_id: String,
    pub user_name: String,
    pub face: String,
    pub title: String,
    pub cover: String,
    /// 最后观看时间（unix 秒）
    pub time: u64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub danmaku: DanmakuStyle,
    pub follows: Vec<Follow>,
    pub history: Vec<History>,
    pub theme: Theme,
    /// 侧边栏页面顺序（HOME_PAGES 的 id）
    pub home_sort: Vec<String>,
    /// 平台顺序（Site::id）
    pub site_sort: Vec<String>,
    pub live: LiveSettings,
    pub follow: FollowSettings,
    pub auto_exit: AutoExit,
    /// 动作 -> 按键
    pub keys: BTreeMap<String, String>,
    pub bili: BiliAccount,
    /// 启动时及每 6 小时检查新版本，后台下载、退出时安装
    pub auto_update: bool,
    /// 也接收预发行版本（目前发布的都是预发行）
    pub update_beta: bool,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            danmaku: DanmakuStyle::default(),
            follows: vec![],
            history: vec![],
            theme: Theme::System,
            home_sort: HOME_PAGES.iter().map(|(id, _)| id.to_string()).collect(),
            site_sort: vec![],
            live: LiveSettings::default(),
            follow: FollowSettings::default(),
            auto_exit: AutoExit::default(),
            keys: default_keys(),
            bili: BiliAccount::default(),
            auto_update: true,
            update_beta: true,
        }
    }
}

/// shortcut: 观看记录最多留 200 条，要更多再改成分页存储
const MAX_HISTORY: usize = 200;

fn path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Library/Application Support/Livo/settings.json")
}

impl Store {
    /// 文件不存在或损坏时用默认值，不让一个坏文件挡住启动；损坏的原文件另存为 settings.json.bad
    pub fn load() -> Self {
        Self::load_from(&path())
    }

    fn load_from(p: &std::path::Path) -> Self {
        let Ok(bytes) = std::fs::read(p) else { return Self::default() };
        let mut s: Self = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            // 坏文件改名留着：不然下一次保存会用默认值把关注、观看记录和登录全部覆盖掉
            let bad = p.with_extension("json.bad");
            eprintln!("设置文件解析失败（{e}），原文件已另存为 {}", bad.display());
            let _ = std::fs::rename(p, &bad);
            Self::default()
        });
        // 页面顺序只留当前版本有的页面（旧版本还有“搜索”），缺的补在后面；设置页按下标移动，两边必须一致
        s.home_sort.retain(|id| HOME_PAGES.iter().any(|(p, _)| p == id));
        for (id, _) in HOME_PAGES {
            if !s.home_sort.iter().any(|x| x == id) {
                s.home_sort.push(id.to_string());
            }
        }
        s
    }

    pub fn save(&self) {
        if let Err(e) = self.save_to(&path()) {
            eprintln!("保存设置失败：{e}");
        }
    }

    /// 先写临时文件再改名，写到一半崩溃也不会把关注列表写坏
    fn save_to(&self, p: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, p)
    }

    /// 记一条观看记录：同一房间只留最新一条，放最前面
    pub fn record(&mut self, h: History) {
        self.history.retain(|x| !(x.site == h.site && x.room_id == h.room_id));
        self.history.insert(0, h);
        self.history.truncate(MAX_HISTORY);
    }

    pub fn key_for(&self, action: &str) -> String {
        self.keys.get(action).cloned().unwrap_or_else(|| KEY_ACTIONS.iter().find(|(a, ..)| *a == action).map_or(String::new(), |(_, _, k)| k.to_string()))
    }

    pub fn is_followed(&self, site: &str, room_id: &str) -> bool {
        self.follows.iter().any(|f| f.site == site && f.room_id == room_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tolerates_bad_files() {
        let dir = std::env::temp_dir().join(format!("livo_store_test_{}", std::process::id()));
        let p = dir.join("settings.json");
        let mut s = Store::default();
        s.danmaku.font = 24.;
        s.follows.push(Follow { site: "huya".into(), room_id: "660000".into(), user_name: "a".into(), face: String::new(), title: String::new(), cover: String::new() });
        s.save_to(&p).unwrap();
        assert_eq!(Store::load_from(&p), s);
        assert!(Store::load_from(&p).is_followed("huya", "660000"));
        // 缺字段用默认值，坏文件回落到默认
        std::fs::write(&p, r#"{"danmaku":{"font":30}}"#).unwrap();
        assert_eq!(Store::load_from(&p).danmaku, DanmakuStyle { font: 30., ..Default::default() });
        // 观看记录去重并把最新的放前面
        let mut s = Store::default();
        let h = |id: &str, time| History { site: "huya".into(), room_id: id.into(), user_name: String::new(), face: String::new(), title: String::new(), cover: String::new(), time };
        s.record(h("1", 1));
        s.record(h("2", 2));
        s.record(h("1", 3));
        assert_eq!(s.history.iter().map(|x| (x.room_id.as_str(), x.time)).collect::<Vec<_>>(), [("1", 3), ("2", 2)]);
        assert_eq!(s.key_for("fullscreen"), "f");
        // 旧版本的页面顺序里有“搜索”：去掉，缺的页面补上
        std::fs::write(&p, r#"{"home_sort":["search","category","recommend"]}"#).unwrap();
        assert_eq!(Store::load_from(&p).home_sort, ["category", "recommend", "follow"]);
        std::fs::write(&p, "not json").unwrap();
        assert_eq!(Store::load_from(&p), Store::default());
        assert_eq!(std::fs::read(p.with_extension("json.bad")).unwrap(), b"not json", "坏文件要原样留着");
        std::fs::remove_dir_all(dir).ok();
    }
}
