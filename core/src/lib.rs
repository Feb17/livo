pub mod bilibili;
pub mod danmaku;
pub mod douyu;
pub mod huya;
pub mod tars;

use anyhow::Result;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoomItem {
    pub room_id: String,
    pub title: String,
    pub cover: String,
    pub user_name: String,
    pub online: i64,
}

#[derive(Clone, Debug, Default)]
pub struct RoomPage {
    pub has_more: bool,
    pub items: Vec<RoomItem>,
}

#[derive(Clone, Debug)]
pub struct SubCategory {
    pub id: String,
    pub parent_id: String,
    pub name: String,
    pub pic: String,
}

#[derive(Clone, Debug)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub children: Vec<SubCategory>,
}

#[derive(Clone, Debug, Default)]
pub struct RoomDetail {
    pub room_id: String,
    pub title: String,
    pub cover: String,
    pub user_name: String,
    pub user_avatar: String,
    pub online: i64,
    pub live: bool,
    pub danmaku: DanmakuArgs,
}

#[derive(Clone, Debug)]
pub struct PlayQuality {
    pub name: String,
    pub qn: i64,
}

#[derive(Clone, Debug, Default)]
pub struct PlayUrl {
    pub urls: Vec<String>,
    pub headers: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub enum LiveMessage {
    Chat { user: String, text: String, color: (u8, u8, u8) },
    Online(i64),
    SuperChat { user: String, text: String, price: i64 },
}

#[derive(Clone, Debug, Default)]
pub enum DanmakuArgs {
    #[default]
    None,
    BiliBili(danmaku::BiliDanmakuArgs),
    /// 斗鱼只需真实房间号
    Douyu(String),
    Huya { ayyuid: i64, top_sid: i64, sub_sid: i64 },
}

/// 对应 Dart 的 LiveSite：各平台按枚举分发，避免 async trait 对象。
pub enum Site {
    BiliBili(bilibili::BiliBili),
    Douyu(douyu::Douyu),
    Huya(huya::Huya),
}

macro_rules! each {
    ($self:ident, $s:ident => $e:expr) => {
        match $self {
            Site::BiliBili($s) => $e,
            Site::Douyu($s) => $e,
            Site::Huya($s) => $e,
        }
    };
}

impl Site {
    pub fn all() -> Vec<Site> {
        vec![Site::BiliBili(bilibili::BiliBili::new()), Site::Douyu(douyu::Douyu::new()), Site::Huya(huya::Huya::new())]
    }

    /// 持久化用的稳定标识（同原项目的 siteId）
    pub fn id(&self) -> &'static str {
        match self {
            Site::BiliBili(_) => "bilibili",
            Site::Douyu(_) => "douyu",
            Site::Huya(_) => "huya",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Site::BiliBili(_) => "哔哩哔哩",
            Site::Douyu(_) => "斗鱼",
            Site::Huya(_) => "虎牙",
        }
    }

    pub async fn categories(&self) -> Result<Vec<Category>> {
        each!(self, s => s.categories().await)
    }
    pub async fn category_rooms(&self, sub: &SubCategory, page: u32) -> Result<RoomPage> {
        each!(self, s => s.category_rooms(sub, page).await)
    }
    pub async fn recommend_rooms(&self, page: u32) -> Result<RoomPage> {
        each!(self, s => s.recommend_rooms(page).await)
    }
    pub async fn search_rooms(&self, keyword: &str, page: u32) -> Result<RoomPage> {
        each!(self, s => s.search_rooms(keyword, page).await)
    }
    pub async fn room_detail(&self, room_id: &str) -> Result<RoomDetail> {
        each!(self, s => s.room_detail(room_id).await)
    }
    pub async fn play_qualities(&self, room_id: &str) -> Result<Vec<PlayQuality>> {
        each!(self, s => s.play_qualities(room_id).await)
    }
    pub async fn play_urls(&self, room_id: &str, qn: i64) -> Result<PlayUrl> {
        each!(self, s => s.play_urls(room_id, qn).await)
    }
}
