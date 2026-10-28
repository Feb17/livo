use crate::bilibili::{i, s, urlenc};
use crate::*;
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const SEARCH_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36 Edg/114.0.1823.51";
const ROOM_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36 Edg/114.0.1823.43";

/// 对应 Dart 的 DouyuSite；匿名访问，设备号每个进程随机一次。
pub struct Douyu {
    http: reqwest::Client,
    did: String,
}

impl Default for Douyu {
    fn default() -> Self {
        Self::new()
    }
}

fn random_did() -> String {
    use std::hash::BuildHasher;
    let h = std::collections::hash_map::RandomState::new();
    format!("{:016x}{:016x}", h.hash_one(1), h.hash_one(2))
}

fn arr(v: &Value) -> &[Value] {
    v.as_array().map(|a| a.as_slice()).unwrap_or_default()
}

fn rooms(v: &Value, page: u32) -> RoomPage {
    let items = arr(&v["data"]["rl"])
        .iter()
        .filter(|x| x["type"] == 1)
        .map(|x| RoomItem { room_id: s(&x["rid"]), title: s(&x["rn"]), cover: s(&x["rs16"]), user_name: s(&x["nn"]), avatar: s(&x["av"]), online: i(&x["ol"]) })
        .collect();
    RoomPage { has_more: (page as i64) < i(&v["data"]["pgcnt"]), items }
}

/// "1.2万" -> 12000
fn hot_num(h: &str) -> i64 {
    let n: f64 = h.replace('万', "").parse().unwrap_or(0.0);
    (if h.contains('万') { n * 10000.0 } else { n }).round() as i64
}

fn html_unescape(v: &str) -> String {
    v.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

/// getEncryption 下发的签名材料，与设备号绑定，可在多次取流间复用。
/// 算法照搬 rust-srec 的 fallback_sign，不需要 JS 引擎。
struct SignKey {
    rand_str: String,
    key: String,
    enc_time: u32,
    is_special: bool,
    enc_data: String,
}

fn auth(k: &SignKey, rid: &str, ts: u64) -> String {
    let mut secret = k.rand_str.clone();
    for _ in 0..k.enc_time {
        secret = format!("{:x}", md5::compute(format!("{secret}{}", k.key)));
    }
    let salt = if k.is_special { String::new() } else { format!("{rid}{ts}") };
    format!("{:x}", md5::compute(format!("{secret}{}{salt}", k.key)))
}

impl Douyu {
    pub fn new() -> Self {
        Self { http: reqwest::Client::builder().build().expect("http client"), did: random_did() }
    }

    fn req(&self, rb: reqwest::RequestBuilder, referer: &str, ua: &str) -> reqwest::RequestBuilder {
        rb.header("referer", referer).header("user-agent", ua).header("cookie", format!("dy_did={0}; acf_did={0}", self.did))
    }

    async fn get(&self, url: &str, query: &[(&str, String)], referer: &str, ua: &str) -> Result<Value> {
        let r = self.req(self.http.get(url).query(query), referer, ua).send().await?;
        if !r.status().is_success() {
            return Err(anyhow!("请求失败 {}: {url}", r.status()));
        }
        Ok(r.json().await?)
    }

    pub async fn categories(&self) -> Result<Vec<Category>> {
        let v = self.get("https://m.douyu.com/api/cate/list", &[], "https://m.douyu.com/", UA).await?;
        let subs = arr(&v["data"]["cate2Info"]);
        let mut cats: Vec<_> = arr(&v["data"]["cate1Info"])
            .iter()
            .map(|c| Category {
                id: s(&c["cate1Id"]),
                name: s(&c["cate1Name"]),
                children: subs
                    .iter()
                    .filter(|x| x["cate1Id"] == c["cate1Id"])
                    .map(|x| SubCategory { id: s(&x["cate2Id"]), parent_id: s(&c["cate1Id"]), name: s(&x["cate2Name"]), pic: s(&x["icon"]) })
                    .collect(),
            })
            .collect();
        cats.sort_by_key(|c| c.id.parse::<i64>().unwrap_or(0));
        Ok(cats)
    }

    pub async fn category_rooms(&self, sub: &SubCategory, page: u32) -> Result<RoomPage> {
        let url = format!("https://www.douyu.com/gapi/rkc/directory/mixList/2_{}/{page}", sub.id);
        Ok(rooms(&self.get(&url, &[], "https://www.douyu.com/", UA).await?, page))
    }

    pub async fn recommend_rooms(&self, page: u32) -> Result<RoomPage> {
        let url = format!("https://www.douyu.com/japi/weblist/apinc/allpage/6/{page}");
        Ok(rooms(&self.get(&url, &[], "https://www.douyu.com/", UA).await?, page))
    }

    pub async fn search_rooms(&self, keyword: &str, page: u32) -> Result<RoomPage> {
        let q = [("kw", keyword.to_string()), ("page", page.to_string()), ("pageSize", "20".into())];
        let v = self.get("https://www.douyu.com/japi/search/api/searchShow", &q, "https://www.douyu.com/search/", SEARCH_UA).await?;
        if v["error"] != 0 {
            return Err(anyhow!("斗鱼搜索失败: {}", s(&v["msg"])));
        }
        let items: Vec<_> = arr(&v["data"]["relateShow"])
            .iter()
            .map(|x| RoomItem { room_id: s(&x["rid"]), title: s(&x["roomName"]), cover: s(&x["roomSrc"]), user_name: s(&x["nickName"]), avatar: s(&x["avatar"]), online: hot_num(&s(&x["hot"])) })
            .collect();
        Ok(RoomPage { has_more: !items.is_empty(), items })
    }

    pub async fn search_anchors(&self, keyword: &str, page: u32) -> Result<(bool, Vec<AnchorItem>)> {
        // filterType=0：包括未开播的主播（1 只返回在播的）
        let q = [("kw", keyword.to_string()), ("page", page.to_string()), ("pageSize", "20".into()), ("filterType", "0".into())];
        let v = self.get("https://www.douyu.com/japi/search/api/searchUser", &q, "https://www.douyu.com/search/", SEARCH_UA).await?;
        if v["error"] != 0 {
            return Err(anyhow!("斗鱼搜索失败: {}", s(&v["msg"])));
        }
        let items: Vec<_> = arr(&v["data"]["relateUser"])
            .iter()
            .map(|x| {
                let a = &x["anchorInfo"];
                // roomType 非 0 的是轮播等特殊房间，同原项目按未开播处理
                AnchorItem { room_id: s(&a["rid"]), avatar: s(&a["avatar"]), user_name: s(&a["nickName"]), live: i(&a["isLive"]) == 1 && i(&a["roomType"]) == 0 }
            })
            .collect();
        Ok((!items.is_empty(), items))
    }

    async fn room_info(&self, room_id: &str) -> Result<Value> {
        let v = self.get(&format!("https://www.douyu.com/betard/{room_id}"), &[], &format!("https://www.douyu.com/{room_id}"), ROOM_UA).await?;
        let room = &v["room"];
        if room["room_id"].is_null() {
            return Err(anyhow!("斗鱼房间数据结构变更或房间不存在，请稍后重试"));
        }
        Ok(room.clone())
    }

    pub async fn room_detail(&self, room_id: &str) -> Result<RoomDetail> {
        let r = self.room_info(room_id).await?;
        let real = s(&r["room_id"]);
        Ok(RoomDetail {
            title: s(&r["room_name"]),
            cover: s(&r["room_pic"]),
            user_name: s(&r["owner_name"]),
            user_avatar: s(&r["owner_avatar"]),
            online: i(&r["room_biz_all"]["hot"]),
            live: r["show_status"] == 1 && r["videoLoop"] != 1,
            start_time: i(&r["show_time"]),
            danmaku: DanmakuArgs::Douyu(real.clone()),
            room_id: real,
        })
    }

    async fn sign_key(&self) -> Result<SignKey> {
        let v = self.get("https://www.douyu.com/wgapi/livenc/liveweb/websec/getEncryption", &[("did", self.did.clone())], "https://www.douyu.com/", UA).await?;
        let d = &v["data"];
        if v["error"] != 0 || d.is_null() {
            return Err(anyhow!("斗鱼签名密钥获取失败：{}", s(&v["msg"])));
        }
        Ok(SignKey {
            rand_str: s(&d["rand_str"]),
            key: s(&d["key"]),
            enc_time: i(&d["enc_time"]) as u32,
            is_special: d["is_special"].as_bool().unwrap_or(i(&d["is_special"]) != 0),
            enc_data: s(&d["enc_data"]),
        })
    }

    /// 每次请求用新时间戳签名，避免 -9 时间戳错误。
    async fn h5_play(&self, room_id: &str, k: &SignKey, cdn: &str, rate: i64) -> Result<Value> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let form = [
            ("enc_data", k.enc_data.clone()),
            ("tt", ts.to_string()),
            ("did", self.did.clone()),
            ("auth", auth(k, room_id, ts)),
            ("cdn", cdn.into()),
            ("rate", rate.to_string()),
            ("ver", "Douyu_new".into()),
            ("iar", "0".into()),
            ("ive", "0".into()),
            ("rid", room_id.into()),
            ("hevc", "0".into()),
            ("fa", "0".into()),
            ("sov", "0".into()),
        ];
        let body = form.iter().map(|(k, v)| format!("{k}={}", urlenc(v))).collect::<Vec<_>>().join("&");
        let rb = self.http.post(format!("https://www.douyu.com/lapi/live/getH5PlayV1/{room_id}")).header("content-type", "application/x-www-form-urlencoded").body(body);
        let v: Value = self.req(rb, "https://www.douyu.com/", UA).send().await?.json().await?;
        if v["error"] != 0 {
            return Err(anyhow!("斗鱼播放请求失败：{}", s(&v["msg"])));
        }
        Ok(v["data"].clone())
    }

    /// 返回 (清晰度, cdn 列表)；scdn 线路排后。
    async fn play_info(&self, room_id: &str, k: &SignKey) -> Result<(Vec<PlayQuality>, Vec<String>)> {
        let d = self.h5_play(room_id, k, "", -1).await?;
        let mut cdns: Vec<_> = arr(&d["cdnsWithName"]).iter().map(|c| s(&c["cdn"])).collect();
        cdns.sort_by_key(|c| c.starts_with("scdn"));
        let qs: Vec<_> = arr(&d["multirates"]).iter().map(|q| PlayQuality { name: s(&q["name"]), qn: i(&q["rate"]) }).collect();
        if qs.is_empty() {
            return Err(anyhow!("斗鱼返回了无效的播放质量"));
        }
        Ok((qs, cdns))
    }

    pub async fn play_qualities(&self, room_id: &str) -> Result<Vec<PlayQuality>> {
        Ok(self.play_info(room_id, &self.sign_key().await?).await?.0)
    }

    pub async fn play_urls(&self, room_id: &str, rate: i64) -> Result<PlayUrl> {
        let k = self.sign_key().await?;
        let (_, cdns) = self.play_info(room_id, &k).await?;
        let mut urls = vec![];
        for cdn in cdns {
            let Ok(d) = self.h5_play(room_id, &k, &cdn, rate).await else { continue };
            let (base, live) = (s(&d["rtmp_url"]), s(&d["rtmp_live"]));
            if base.is_empty() || live.is_empty() {
                continue;
            }
            let mut url = format!("{base}/{}", html_unescape(&live));
            // ws 线路默认 expire=300，几分钟就断，追加 expire=0
            if url.contains("expire=300") && url.contains("fcdn=ws") {
                url.push_str("&expire=0");
            }
            urls.push(url);
        }
        if urls.is_empty() {
            return Err(anyhow!("斗鱼返回了无效的播放地址"));
        }
        Ok(PlayUrl { urls, headers: vec![("referer".into(), "https://www.douyu.com/".into()), ("user-agent".into(), UA.into())] })
    }
}

/// 斗鱼包：长度(LE u32)×2、类型 689(LE u16)、加密 0、保留 0、正文、结尾 0。
pub fn encode(body: &str) -> Vec<u8> {
    let len = (9 + body.len()) as u32;
    let mut out = Vec::with_capacity(4 + len as usize);
    out.extend(len.to_le_bytes());
    out.extend(len.to_le_bytes());
    out.extend(689u16.to_le_bytes());
    out.extend([0, 0]);
    out.extend(body.as_bytes());
    out.push(0);
    out
}

/// 拆出一帧里的所有 STT 正文。
pub fn decode(data: &[u8]) -> Vec<String> {
    let mut out = vec![];
    let mut pos = 0;
    while pos + 12 < data.len() {
        let len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        if len < 9 || pos + 4 + len > data.len() {
            break;
        }
        out.push(String::from_utf8_lossy(&data[pos + 12..pos + 3 + len]).into_owned());
        pos += 4 + len;
    }
    out
}

/// 只取顶层 key@=value，嵌套字段保持原样（用不到）。
fn stt_get(stt: &str, key: &str) -> Option<String> {
    stt.split('/').find_map(|f| f.split_once("@=").filter(|(k, _)| *k == key).map(|(_, v)| v.replace("@S", "/").replace("@A", "@")))
}

pub fn parse_message(stt: &str) -> Option<LiveMessage> {
    // 没有 dms 的是“阴间弹幕”，Dart 版同样屏蔽
    if stt_get(stt, "type")? != "chatmsg" || stt_get(stt, "dms").is_none() {
        return None;
    }
    let color = match stt_get(stt, "col").and_then(|c| c.parse().ok()).unwrap_or(0) {
        1 => (255, 0, 0),
        2 => (30, 135, 240),
        3 => (122, 200, 75),
        4 => (255, 127, 0),
        5 => (155, 57, 244),
        6 => (255, 105, 180),
        _ => (255, 255, 255),
    };
    Some(LiveMessage::Chat { user: stt_get(stt, "nn")?, text: stt_get(stt, "txt")?, color })
}

/// 服务器只给 AES256-GCM-SHA384（无前向保密），rustls 不支持，所以 ws 用 native-tls。
/// shortcut: 只连 8501 端口、不重连，Dart 版会在 8501-8506 间轮换；断线多了再加。
pub async fn run_danmaku(room_id: String, tx: UnboundedSender<LiveMessage>) -> Result<()> {
    let (ws, _) = tokio_tungstenite::connect_async("wss://danmuproxy.douyu.com:8501").await?;
    let (mut w, mut r) = ws.split();
    w.send(Message::Binary(encode(&format!("type@=loginreq/roomid@={room_id}/")))).await?;
    w.send(Message::Binary(encode(&format!("type@=joingroup/rid@={room_id}/gid@=-9999/")))).await?;
    let mut beat = tokio::time::interval(Duration::from_secs(45));
    loop {
        tokio::select! {
            _ = beat.tick() => w.send(Message::Binary(encode("type@=mrkl/"))).await?,
            m = r.next() => match m {
                Some(Ok(Message::Binary(b))) => {
                    for m in decode(&b).iter().filter_map(|x| parse_message(x)) {
                        if tx.send(m).is_err() { return Ok(()); }
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_chatmsg() {
        let mut frame = encode("type@=chatmsg/nn@=小明/txt@=a@Sb/col@=2/dms@=3/");
        frame.extend(encode("type@=chatmsg/nn@=x/txt@=阴间/"));
        let msgs: Vec<_> = decode(&frame).iter().filter_map(|x| parse_message(x)).collect();
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            LiveMessage::Chat { user, text, color, .. } => assert_eq!((user.as_str(), text.as_str(), *color), ("小明", "a/b", (30, 135, 240))),
            m => panic!("{m:?}"),
        }
    }

    #[test]
    fn hot() {
        assert_eq!(hot_num("1.2万"), 12000);
        assert_eq!(hot_num("345"), 345);
    }

    #[test]
    fn auth_iterates_md5_and_salts() {
        let md5 = |x: String| format!("{:x}", md5::compute(x));
        let mut k = SignKey { rand_str: "r".into(), key: "k".into(), enc_time: 2, is_special: false, enc_data: String::new() };
        let secret = md5(md5("rk".into()) + "k");
        assert_eq!(auth(&k, "99", 7), md5(format!("{secret}k997")));
        k.is_special = true;
        assert_eq!(auth(&k, "99", 7), md5(format!("{secret}k")));
    }
}
