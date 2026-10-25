use crate::danmaku::BiliDanmakuArgs;
use crate::*;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0";
const ROOM_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36 Edg/115.0.1901.188";
const MIXIN_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

/// 对应 Dart 的 BiliBiliSite；cookie 为空时只用匿名 buvid。
pub struct BiliBili {
    http: reqwest::Client,
    pub cookie: String,
    pub uid: i64,
    buvid: Mutex<Option<(String, String)>>,
    wbi: Mutex<Option<(String, String)>>,
}

impl Default for BiliBili {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn s(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

/// 宽松取整数：虎牙等接口会把整数写成 1.0 / 3.7E7，或者写成字符串。
pub(crate) fn i(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_str().and_then(|x| x.parse().ok())).unwrap_or(0)
}

/// wbi 签名：参数排序、剔除 !'()*、拼 mixin key 后取 md5。
pub fn wbi_sign(params: &[(&str, String)], img: &str, sub: &str, wts: u64) -> Vec<(String, String)> {
    let raw: Vec<char> = format!("{img}{sub}").chars().collect();
    let mixin: String = MIXIN_TAB.iter().map(|&n| raw[n]).take(32).collect();
    let mut map: BTreeMap<String, String> = params.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    map.insert("wts".into(), wts.to_string());
    for v in map.values_mut() {
        v.retain(|c| !"!'()*".contains(c));
    }
    let query = map.iter().map(|(k, v)| format!("{k}={}", urlenc(v))).collect::<Vec<_>>().join("&");
    let rid = format!("{:x}", md5::compute(format!("{query}{mixin}")));
    let mut out: Vec<_> = map.into_iter().collect();
    out.push(("w_rid".into(), rid));
    out
}

pub(crate) fn urlenc(v: &str) -> String {
    v.bytes()
        .map(|b| match b {
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn room_from(item: &Value, cover: String) -> RoomItem {
    RoomItem {
        room_id: s(&item["roomid"]),
        title: s(&item["title"]),
        cover,
        user_name: s(&item["uname"]),
        online: i(&item["online"]),
    }
}

/// 扫码登录的轮询结果（同原项目 QRStatus）
#[derive(Debug, PartialEq)]
pub enum QrPoll {
    Waiting,
    Scanned,
    Expired,
    /// 登录成功，拿到的 Cookie
    Done(String),
}

impl BiliBili {
    /// 带登录 Cookie 的实例；登录后的接口（如高清晰度）会用上它
    pub fn with_cookie(cookie: String, uid: i64) -> Self {
        Self { cookie, uid, ..Self::new() }
    }

    pub async fn qr_generate(&self) -> Result<(String, String)> {
        let v: Value = self.http.get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate").header("user-agent", UA).send().await?.json().await?;
        let (url, key) = (s(&v["data"]["url"]), s(&v["data"]["qrcode_key"]));
        if url.is_empty() || key.is_empty() {
            return Err(anyhow!("获取登录二维码失败：{}", s(&v["message"])));
        }
        Ok((url, key))
    }

    pub async fn qr_poll(&self, key: &str) -> Result<QrPoll> {
        let r = self.http.get("https://passport.bilibili.com/x/passport-login/web/qrcode/poll").query(&[("qrcode_key", key)]).header("user-agent", UA).send().await?;
        // 登录成功时 Cookie 在响应头里，同原项目取每条 set-cookie 的第一段
        let cookie = r.headers().get_all("set-cookie").iter().filter_map(|h| h.to_str().ok()?.split(';').next().map(str::to_string)).collect::<Vec<_>>().join(";");
        let v: Value = r.json().await?;
        Ok(match i(&v["data"]["code"]) {
            0 if !cookie.is_empty() => QrPoll::Done(cookie),
            86038 => QrPoll::Expired,
            86090 => QrPoll::Scanned,
            _ => QrPoll::Waiting,
        })
    }

    /// 用 Cookie 查当前账号 (uid, 昵称)；未登录返回错误
    pub async fn user_info(&self, cookie: &str) -> Result<(i64, String)> {
        let v: Value = self.http.get("https://api.bilibili.com/x/web-interface/nav").header("user-agent", UA).header("cookie", cookie).send().await?.json().await?;
        if v["data"]["isLogin"] != true {
            return Err(anyhow!("B站登录已失效"));
        }
        Ok((i(&v["data"]["mid"]), s(&v["data"]["uname"])))
    }

    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder().build().expect("http client"),
            cookie: String::new(),
            uid: 0,
            buvid: Mutex::new(None),
            wbi: Mutex::new(None),
        }
    }

    async fn buvid(&self) -> (String, String) {
        let mut g = self.buvid.lock().await;
        if let Some(b) = g.as_ref() {
            return b.clone();
        }
        let r = self
            .http
            .get("https://api.bilibili.com/x/frontend/finger/spi")
            .header("user-agent", UA)
            .header("referer", "https://live.bilibili.com/")
            .send()
            .await;
        let v: Value = match r {
            Ok(r) => r.json().await.unwrap_or(Value::Null),
            Err(_) => Value::Null,
        };
        let b = (s(&v["data"]["b_3"]), s(&v["data"]["b_4"]));
        if !b.0.is_empty() {
            *g = Some(b.clone());
        }
        b
    }

    async fn get(&self, url: &str, query: &[(impl AsRef<str>, impl AsRef<str>)], ua: &str) -> Result<Value> {
        let (b3, b4) = self.buvid().await;
        let cookie = if self.cookie.contains("buvid3") {
            self.cookie.clone()
        } else {
            format!("{};buvid3={b3};buvid4={b4};", self.cookie)
        };
        let q: Vec<_> = query.iter().map(|(k, v)| (k.as_ref(), v.as_ref())).collect();
        let resp = self
            .http
            .get(url)
            .query(&q)
            .header("user-agent", ua)
            .header("referer", "https://live.bilibili.com/")
            .header("cookie", cookie)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow!("请求失败 {status}: {url}"));
        }
        Ok(resp.json().await?)
    }

    async fn wbi_get(&self, url: &str, params: &[(&str, String)]) -> Result<Value> {
        let (img, sub) = self.wbi_keys().await?;
        let wts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let signed = wbi_sign(params, &img, &sub, wts);
        self.get(url, &signed, UA).await
    }

    async fn wbi_keys(&self) -> Result<(String, String)> {
        let mut g = self.wbi.lock().await;
        if let Some(k) = g.as_ref() {
            return Ok(k.clone());
        }
        let v = self.get("https://api.bilibili.com/x/web-interface/nav", &[] as &[(&str, &str)], UA).await?;
        let stem = |u: &Value| {
            let u = s(u);
            let name = u.rsplit('/').next().unwrap_or("").to_string();
            name.split('.').next().unwrap_or("").to_string()
        };
        let k = (stem(&v["data"]["wbi_img"]["img_url"]), stem(&v["data"]["wbi_img"]["sub_url"]));
        if k.0.len() + k.1.len() < 64 {
            return Err(anyhow!("wbi key 获取失败"));
        }
        *g = Some(k.clone());
        Ok(k)
    }

    pub async fn categories(&self) -> Result<Vec<Category>> {
        let v = self
            .get("https://api.live.bilibili.com/room/v1/Area/getList", &[("need_entrance", "1"), ("parent_id", "0")], UA)
            .await?;
        Ok(v["data"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|c| Category {
                id: s(&c["id"]),
                name: s(&c["name"]),
                children: c["list"]
                    .as_array()
                    .map(|a| a.as_slice())
                    .unwrap_or_default()
                    .iter()
                    .map(|x| SubCategory {
                        id: s(&x["id"]),
                        parent_id: s(&x["parent_id"]),
                        name: s(&x["name"]),
                        pic: format!("{}@100w.png", s(&x["pic"])),
                    })
                    .collect(),
            })
            .collect())
    }

    pub async fn category_rooms(&self, sub: &SubCategory, page: u32) -> Result<RoomPage> {
        let v = self
            .get(
                "https://api.live.bilibili.com/room/v1/Area/getRoomList",
                &[
                    ("platform", "web".to_string()),
                    ("parent_area_id", sub.parent_id.clone()),
                    ("area_id", sub.id.clone()),
                    ("page", page.to_string()),
                    ("page_size", "30".into()),
                ],
                UA,
            )
            .await?;
        let data = v["data"].as_array().map(|a| a.as_slice()).unwrap_or_default();
        let items = data
            .iter()
            .map(|it| {
                let cover = ["cover", "user_cover", "system_cover"].iter().map(|k| s(&it[*k])).find(|c| !c.is_empty()).unwrap_or_default();
                room_from(it, if cover.is_empty() { cover } else { format!("{cover}@400w.jpg") })
            })
            .collect();
        Ok(RoomPage { has_more: data.len() >= 30, items })
    }

    pub async fn recommend_rooms(&self, page: u32) -> Result<RoomPage> {
        let v = self
            .wbi_get(
                "https://api.live.bilibili.com/xlive/web-interface/v1/second/getListByArea",
                &[("platform", "web".into()), ("sort", "online".into()), ("page_size", "30".into()), ("page", page.to_string())],
            )
            .await?;
        let list = v["data"]["list"].as_array().ok_or_else(|| anyhow!("推荐接口返回异常: {}", s(&v["message"])))?;
        let items = list.iter().map(|it| room_from(it, format!("{}@400w.jpg", s(&it["cover"])))).collect();
        Ok(RoomPage { has_more: !list.is_empty(), items })
    }

    pub async fn search_rooms(&self, keyword: &str, page: u32) -> Result<RoomPage> {
        let v = self.search("live", keyword, page).await?;
        let em = regex::Regex::new(r"<.*?em.*?>")?;
        let items: Vec<_> = v["data"]["result"]["live_room"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|it| {
                let mut r = room_from(it, format!("https:{}@400w.jpg", s(&it["cover"])));
                r.title = em.replace_all(&r.title, "").into_owned();
                r
            })
            .collect();
        Ok(RoomPage { has_more: items.len() >= 40, items })
    }

    pub async fn search_anchors(&self, keyword: &str, page: u32) -> Result<(bool, Vec<AnchorItem>)> {
        let v = self.search("live_user", keyword, page).await?;
        let em = regex::Regex::new(r"<.*?em.*?>")?;
        let items: Vec<_> = v["data"]["result"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|it| AnchorItem {
                room_id: s(&it["roomid"]),
                avatar: format!("https:{}@400w.jpg", s(&it["uface"])),
                user_name: em.replace_all(&s(&it["uname"]), "").into_owned(),
                live: it["is_live"].as_bool().unwrap_or(false),
            })
            .collect();
        Ok((items.len() >= 40, items))
    }

    async fn search(&self, kind: &str, keyword: &str, page: u32) -> Result<Value> {
        self.get(
            "https://api.bilibili.com/x/web-interface/search/type",
            &[
                ("context", ""),
                ("search_type", kind),
                ("cover_type", "user_cover"),
                ("order", ""),
                ("keyword", keyword),
                ("category_id", ""),
                ("__refresh__", ""),
                ("_extra", ""),
                ("highlight", "0"),
                ("single_column", "0"),
                ("page", &page.to_string()),
            ],
            UA,
        )
        .await
    }

    async fn room_info(&self, room_id: &str) -> Result<Value> {
        let v = self.wbi_get("https://api.live.bilibili.com/xlive/web-room/v1/index/getInfoByRoom", &[("room_id", room_id.to_string())]).await?;
        if v["data"].is_null() {
            return Err(anyhow!("房间信息获取失败: {}", s(&v["message"])));
        }
        Ok(v["data"].clone())
    }

    pub async fn room_detail(&self, room_id: &str) -> Result<RoomDetail> {
        let info = self.room_info(room_id).await?;
        let real = s(&info["room_info"]["room_id"]);
        // 弹幕信息被拦截不应阻止进入直播间
        let danmu = self
            .wbi_get("https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo", &[("id", real.clone())])
            .await
            .map(|v| v["data"].clone())
            .unwrap_or(Value::Null);
        let host = danmu["host_list"][0]["host"].as_str().filter(|h| !h.is_empty()).unwrap_or("broadcastlv.chat.bilibili.com");
        let ri = &info["room_info"];
        Ok(RoomDetail {
            title: s(&ri["title"]),
            cover: s(&ri["cover"]),
            user_name: s(&info["anchor_info"]["base_info"]["uname"]),
            user_avatar: format!("{}@100w.jpg", s(&info["anchor_info"]["base_info"]["face"])),
            online: i(&ri["online"]),
            introduction: s(&ri["description"]),
            live: i(&ri["live_status"]) == 1,
            url: format!("https://live.bilibili.com/{room_id}"),
            danmaku: DanmakuArgs::BiliBili(BiliDanmakuArgs {
                room_id: real.parse().unwrap_or(0),
                uid: self.uid,
                token: s(&danmu["token"]),
                server_host: host.into(),
                buvid: self.buvid().await.0,
                cookie: self.cookie.clone(),
            }),
            room_id: real,
        })
    }

    async fn play_info(&self, q: Vec<(&str, String)>) -> Result<Value> {
        for delay in [800u64, 1600, 0] {
            match self.get("https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo", &q, ROOM_UA).await {
                Err(e) if delay > 0 && e.to_string().contains("429") => tokio::time::sleep(std::time::Duration::from_millis(delay)).await,
                r => return r,
            }
        }
        unreachable!()
    }

    pub async fn play_qualities(&self, room_id: &str) -> Result<Vec<PlayQuality>> {
        let v = self
            .play_info(vec![
                ("room_id", room_id.into()),
                ("protocol", "0,1".into()),
                ("format", "0,1,2".into()),
                ("codec", "0,1".into()),
                ("platform", "web".into()),
            ])
            .await?;
        let pu = &v["data"]["playurl_info"]["playurl"];
        if !pu.is_object() {
            return Err(anyhow!("B站播放信息响应异常，请稍后重试"));
        }
        let names: BTreeMap<i64, String> = pu["g_qn_desc"].as_array().map(|a| a.as_slice()).unwrap_or_default().iter().map(|x| (i(&x["qn"]), s(&x["desc"]))).collect();
        let qs: Vec<_> = pu["stream"][0]["format"][0]["codec"][0]["accept_qn"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|q| PlayQuality { qn: i(q), name: names.get(&i(q)).cloned().unwrap_or_else(|| "未知清晰度".into()) })
            .collect();
        if qs.is_empty() {
            return Err(anyhow!("B站暂时无法获取播放清晰度，请稍后重试"));
        }
        Ok(qs)
    }

    pub async fn play_urls(&self, room_id: &str, qn: i64) -> Result<PlayUrl> {
        let v = self
            .play_info(vec![
                ("room_id", room_id.into()),
                ("protocol", "0,1".into()),
                ("format", "0,2".into()),
                ("codec", "0".into()),
                ("platform", "web".into()),
                ("qn", qn.to_string()),
            ])
            .await?;
        let mut urls = vec![];
        for st in v["data"]["playurl_info"]["playurl"]["stream"].as_array().map(|a| a.as_slice()).unwrap_or_default() {
            for f in st["format"].as_array().map(|a| a.as_slice()).unwrap_or_default() {
                for c in f["codec"].as_array().map(|a| a.as_slice()).unwrap_or_default() {
                    for u in c["url_info"].as_array().map(|a| a.as_slice()).unwrap_or_default() {
                        urls.push(format!("{}{}{}", s(&u["host"]), s(&c["base_url"]), s(&u["extra"])));
                    }
                }
            }
        }
        urls.sort_by_key(|u| u.contains("mcdn")); // mcdn 排后
        Ok(PlayUrl { urls, headers: vec![("referer".into(), "https://live.bilibili.com".into()), ("user-agent".into(), ROOM_UA.into())] })
    }

    pub async fn live_status(&self, room_id: &str) -> Result<bool> {
        let v = self.get("https://api.live.bilibili.com/room/v1/Room/get_info", &[("room_id", room_id)], UA).await?;
        Ok(i(&v["data"]["live_status"]) == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_parsing_accepts_floats_and_strings() {
        let v: Value = serde_json::from_str(r#"[1, 1.0, 3.7274124E7, "42", null]"#).unwrap();
        assert_eq!([i(&v[0]), i(&v[1]), i(&v[2]), i(&v[3]), i(&v[4])], [1, 1, 37274124, 42, 0]);
    }

    #[test]
    fn wbi_sign_is_deterministic() {
        // B站官方文档示例：img/sub key 与参数已知，w_rid 应为 8f6f2b5b3d485fe1886cec6a0be8c5d4
        let p = [("foo", "114".to_string()), ("bar", "514".to_string()), ("zab", "1919810".to_string())];
        let r = wbi_sign(&p, "7cd084941338484aae1ad9425b84077c", "4932caff0ff746eab6f01bf08b70ac45", 1702204169);
        let rid = &r.iter().find(|(k, _)| k == "w_rid").unwrap().1;
        assert_eq!(rid, "8f6f2b5b3d485fe1886cec6a0be8c5d4");
    }
}
