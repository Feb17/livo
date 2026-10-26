use crate::bilibili::{i, s, urlenc};
use crate::tars::{self, Writer};
use crate::*;
use anyhow::{anyhow, Result};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
/// 房间页走移动端指纹
const MOBILE_UA: &str = "Mozilla/5.0 (Linux; Android 11; Pixel 5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.91 Mobile Safari/537.36 Edg/117.0.0.0";
/// wup 接口和拉流都用 HYSDK 客户端指纹
const HYSDK_UA: &str = "HYSDK(Windows, 30000002)_APP(pc_exe&7060000&official)_SDK(trans&2.32.3.5646)";
const BUSS_TYPES: [(&str, &str); 4] = [("1", "网游"), ("2", "单机"), ("8", "娱乐"), ("3", "手游")];

/// 对应 Dart 的 HuyaSite。
pub struct Huya {
    http: reqwest::Client,
}

impl Default for Huya {
    fn default() -> Self {
        Self::new()
    }
}

fn arr(v: &Value) -> &[Value] {
    v.as_array().map(|a| a.as_slice()).unwrap_or_default()
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn md5_hex(s: &str) -> String {
    format!("{:x}", md5::compute(s))
}

/// 低 32 位循环左移 8 位，高位不变。
fn rotl64(t: i64) -> i64 {
    let low = (t as u64 & 0xFFFF_FFFF) as u32;
    (t & !0xFFFF_FFFF) | low.rotate_left(8) as i64
}

fn pct_decode(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut k = 0;
    while k < b.len() {
        match b[k] {
            b'%' if k + 2 < b.len() => match std::str::from_utf8(&b[k + 1..k + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()).ok_or(()) {
                Ok(x) => {
                    out.push(x);
                    k += 2;
                }
                Err(_) => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        k += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_get(q: &str, key: &str) -> Option<String> {
    q.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| pct_decode(v)))
}

/// 按 getCdnTokenInfoEx 下发的 token 重新算 wsSecret（Dart buildAntiCode 的移植）。
fn build_anti_code(stream: &str, presenter_uid: i64, anti: &str, now: i64) -> String {
    let Some(fm) = query_get(anti, "fm") else { return anti.to_string() };
    let ctype = query_get(anti, "ctype").unwrap_or_else(|| "huya_pc_exe".into());
    let platform: i64 = query_get(anti, "t").and_then(|t| t.parse().ok()).unwrap_or(0);
    let is_wap = platform == 103;
    let seq_id = presenter_uid + now;
    let secret_hash = md5_hex(&format!("{seq_id}|{ctype}|{platform}"));
    let convert_uid = rotl64(presenter_uid);
    let calc_uid = if is_wap { presenter_uid } else { convert_uid };
    let prefix = base64::engine::general_purpose::STANDARD.decode(pct_decode(&fm)).ok().and_then(|b| String::from_utf8(b).ok()).unwrap_or_default();
    let prefix = prefix.split('_').next().unwrap_or_default();
    let ws_time = query_get(anti, "wsTime").unwrap_or_default();
    let ws_secret = md5_hex(&format!("{prefix}_{calc_uid}_{stream}_{secret_hash}_{ws_time}"));
    let mut out = vec![
        ("wsSecret", ws_secret),
        ("wsTime", ws_time.clone()),
        ("seqid", seq_id.to_string()),
        ("ctype", ctype),
        ("ver", "1".into()),
        ("fs", query_get(anti, "fs").unwrap_or_default()),
        ("fm", urlenc(&fm)),
        ("t", platform.to_string()),
    ];
    if is_wap {
        let ct = i64::from_str_radix(&ws_time, 16).unwrap_or(0) * 1000;
        out.extend([("uid", presenter_uid.to_string()), ("uuid", ((ct % 10_000_000_000 * 1000) % 0xFFFF_FFFF).to_string())]);
    } else {
        out.push(("u", convert_uid.to_string()));
    }
    out.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&")
}

fn room_item(x: &Value, prefix: &str) -> RoomItem {
    let mut cover = s(&x[format!("{prefix}screenshot")]);
    if !cover.contains('?') {
        cover.push_str("?x-oss-process=style/w338_h190&");
    }
    let mut title = s(&x[format!("{prefix}introduction")]);
    if title.is_empty() {
        title = s(&x[format!("{prefix}roomName")]);
    }
    RoomItem { room_id: s(&x["room_id"]), title, cover, user_name: s(&x[format!("{prefix}nick")]), online: i(&x[format!("{prefix}total_count")]) }
}

fn list_rooms(v: &Value) -> RoomPage {
    let d = &v["data"];
    let items = arr(&d["datas"])
        .iter()
        .map(|x| {
            let mut r = room_item(x, "");
            r.room_id = s(&x["profileRoom"]);
            r.online = i(&x["totalCount"]);
            r
        })
        .collect();
    RoomPage { has_more: i(&d["page"]) < i(&d["totalPage"]), items }
}

/// 递归找第一个名字匹配（忽略大小写）的正整数字段，页面结构挪位时兜底。
fn find_positive(v: &Value, keys: &[&str], depth: u8) -> i64 {
    if depth > 8 {
        return 0;
    }
    match v {
        Value::Object(m) => m
            .iter()
            .find_map(|(k, x)| Some(i(x)).filter(|n| *n > 0 && keys.contains(&k.to_lowercase().as_str())))
            .or_else(|| m.values().map(|x| find_positive(x, keys, depth + 1)).find(|n| *n > 0))
            .unwrap_or(0),
        Value::Array(a) => a.iter().map(|x| find_positive(x, keys, depth + 1)).find(|n| *n > 0).unwrap_or(0),
        _ => 0,
    }
}

impl Huya {
    pub fn new() -> Self {
        Self { http: reqwest::Client::builder().build().expect("http client") }
    }

    async fn get_json(&self, url: &str, query: &[(&str, String)]) -> Result<Value> {
        let r = self.http.get(url).query(query).header("user-agent", UA).send().await?;
        if !r.status().is_success() {
            return Err(anyhow!("请求失败 {}: {url}", r.status()));
        }
        // cache.php 等接口的 content-type 不是 json，按文本解析
        Ok(serde_json::from_str(&r.text().await?)?)
    }

    pub async fn categories(&self) -> Result<Vec<Category>> {
        let subs = futures_util::future::try_join_all(BUSS_TYPES.iter().map(|(id, _)| async move { self.get_json("https://live.cdn.huya.com/liveconfig/game/bussLive", &[("bussType", id.to_string())]).await })).await?;
        Ok(BUSS_TYPES
            .iter()
            .zip(subs)
            .map(|((id, name), v)| Category {
                id: id.to_string(),
                name: name.to_string(),
                children: arr(&v["data"])
                    .iter()
                    .map(|x| {
                        // gid 可能是数字，也可能是 {"value": "a,b"}
                        let gid = match &x["gid"] {
                            Value::Object(o) => s(&o["value"]).split(',').next().unwrap_or_default().to_string(),
                            g => i(g).to_string(),
                        };
                        SubCategory { pic: format!("https://huyaimg.msstatic.com/cdnimage/game/{gid}-MS.jpg"), id: gid, parent_id: id.to_string(), name: s(&x["gameFullName"]) }
                    })
                    .collect(),
            })
            .collect())
    }

    async fn live_list(&self, game_id: Option<&str>, page: u32) -> Result<RoomPage> {
        let mut q = vec![("m", "LiveList".into()), ("do", "getLiveListByPage".into()), ("tagAll", "0".into()), ("page", page.to_string())];
        if let Some(g) = game_id {
            q.push(("gameId", g.into()));
        }
        Ok(list_rooms(&self.get_json("https://www.huya.com/cache.php", &q).await?))
    }

    pub async fn category_rooms(&self, sub: &SubCategory, page: u32) -> Result<RoomPage> {
        self.live_list(Some(&sub.id), page).await
    }

    pub async fn recommend_rooms(&self, page: u32) -> Result<RoomPage> {
        self.live_list(None, page).await
    }

    /// 接口的 start 不是偏移量，固定 start=0、用 rows 扩大累积窗口再截取当前页（同 Dart）。
    pub async fn search_rooms(&self, keyword: &str, page: u32) -> Result<RoomPage> {
        const SIZE: usize = 20;
        let q = [
            ("m", "Search".into()),
            ("do", "getSearchContent".into()),
            ("q", keyword.into()),
            ("uid", "0".into()),
            ("v", "4".into()),
            ("typ", "-5".into()),
            ("livestate", "0".into()),
            ("rows", (page as usize * SIZE).to_string()),
            ("start", "0".into()),
        ];
        let v = self.get_json("https://search.cdn.huya.com/", &q).await?;
        let r = &v["response"]["3"];
        let items = arr(&r["docs"]).iter().skip((page as usize - 1) * SIZE).map(|x| room_item(x, "game_")).collect();
        Ok(RoomPage { has_more: i(&r["numFound"]) > (page as usize * SIZE) as i64, items })
    }

    /// 主播在 response["1"]，直播间在 response["3"]；分页方式同 search_rooms
    pub async fn search_anchors(&self, keyword: &str, page: u32) -> Result<(bool, Vec<AnchorItem>)> {
        const SIZE: usize = 20;
        let q = [
            ("m", "Search".into()),
            ("do", "getSearchContent".into()),
            ("q", keyword.into()),
            ("uid", "0".into()),
            ("v", "1".into()),
            ("typ", "-5".into()),
            ("livestate", "0".into()),
            ("rows", (page as usize * SIZE).to_string()),
            ("start", "0".into()),
        ];
        let v = self.get_json("https://search.cdn.huya.com/", &q).await?;
        let r = &v["response"]["1"];
        let items = arr(&r["docs"])
            .iter()
            .skip((page as usize - 1) * SIZE)
            .map(|x| AnchorItem { room_id: s(&x["room_id"]), avatar: s(&x["game_avatarUrl180"]), user_name: s(&x["game_nick"]), live: x["gameLiveOn"].as_bool().unwrap_or(false) })
            .collect();
        Ok((i(&r["numFound"]) > (page as usize * SIZE) as i64, items))
    }

    /// 解析移动端房间页里的 HNF_GLOBAL_INIT，附带 topSid/subSid。
    async fn room_info(&self, room_id: &str) -> Result<(Value, i64, i64)> {
        let html = self.http.get(format!("https://m.huya.com/{room_id}")).header("user-agent", MOBILE_UA).send().await?.text().await?;
        let raw = regex::Regex::new(r"window\.HNF_GLOBAL_INIT.=.\{[\s\S]*?\}[\s\S]*?</script>")?
            .find(&html)
            .ok_or_else(|| anyhow!("虎牙页面数据结构变更或访问受限，请稍后重试"))?
            .as_str();
        let text = regex::Regex::new(r"window\.HNF_GLOBAL_INIT.=.")?.replace(raw, "").replace("</script>", "");
        let text = regex::Regex::new(r"function.*?\(.*?\).\{[\s\S]*?\}")?.replace_all(&text, "\"\"");
        let v: Value = serde_json::from_str(&text)?;
        let grab = |re: &str, keys: &[&str]| {
            regex::Regex::new(re).ok().and_then(|r| r.captures(&html)).and_then(|c| c[1].parse().ok()).filter(|n: &i64| *n > 0).unwrap_or_else(|| find_positive(&v, keys, 0))
        };
        let top = grab(r#"lChannelId":([0-9]+)"#, &["lchannelid", "channelid"]);
        let sub = grab(r#"lSubChannelId":([0-9]+)"#, &["lsubchannelid", "subchannelid"]);
        if !v["roomInfo"]["tLiveInfo"].is_object() || !v["roomInfo"]["tProfileInfo"].is_object() {
            return Err(anyhow!("虎牙房间数据结构变更，请联系开发者适配"));
        }
        Ok((v, top, sub))
    }

    pub async fn room_detail(&self, room_id: &str) -> Result<RoomDetail> {
        let (v, top, sub) = self.room_info(room_id).await?;
        let (live, profile) = (&v["roomInfo"]["tLiveInfo"], &v["roomInfo"]["tProfileInfo"]);
        let mut title = s(&live["sIntroduction"]);
        if title.is_empty() {
            title = s(&live["sRoomName"]);
        }
        Ok(RoomDetail {
            room_id: s(&live["lProfileRoom"]),
            title,
            cover: s(&live["sScreenshot"]),
            user_name: s(&profile["sNick"]),
            user_avatar: s(&profile["sAvatar180"]),
            online: i(&live["lTotalCount"]),
            live: v["roomInfo"]["eLiveStatus"] == 2,
            danmaku: DanmakuArgs::Huya { ayyuid: i(&live["lYyid"]), top_sid: top, sub_sid: sub },
        })
    }

    pub async fn play_qualities(&self, room_id: &str) -> Result<Vec<PlayQuality>> {
        let (v, _, _) = self.room_info(room_id).await?;
        let mut qs: Vec<_> = arr(&v["roomInfo"]["tLiveInfo"]["tLiveStreamInfo"]["vBitRateInfo"]["value"])
            .iter()
            .map(|x| PlayQuality { name: s(&x["sDisplayName"]), qn: i(&x["iBitRate"]) })
            .filter(|q| !q.name.contains("HDR"))
            .collect();
        if qs.is_empty() {
            qs = vec![PlayQuality { name: "原画".into(), qn: 0 }, PlayQuality { name: "高清".into(), qn: 2000 }];
        }
        Ok(qs)
    }

    /// wup 的 getCdnTokenInfoEx：拿到带 fm/wsTime 的 flv token。
    async fn cdn_token(&self, stream: &str) -> Result<String> {
        let mut id = Writer::default();
        id.int(0, 0).string(1, "").string(2, "").string(3, "pc_exe&7060000&official").string(4, "").int(5, 0).string(6, "").string(7, "");
        let mut req = Writer::default();
        req.string(0, "").string(1, stream).int(2, 0).struct_(3, &id).int(4, 66);
        let body = tars::tup_request("liveui", "getCdnTokenInfoEx", &req);
        let r = self
            .http
            .post("http://wup.huya.com")
            .header("content-type", "application/x-wup")
            .header("origin", "https://m.huya.com/")
            .header("referer", "https://m.huya.com/")
            .header("user-agent", HYSDK_UA)
            .body(body)
            .send()
            .await?
            .bytes()
            .await?;
        let (code, rsp) = tars::tup_response(&r);
        let token = rsp.string(0);
        if code != 0 || token.is_empty() {
            return Err(anyhow!("虎牙取流 token 失败（{code}）"));
        }
        Ok(token)
    }

    pub async fn play_urls(&self, room_id: &str, bit_rate: i64) -> Result<PlayUrl> {
        let (v, top, sub) = self.room_info(room_id).await?;
        let uid = if top > 0 { top } else { sub };
        let mut urls = vec![];
        for line in arr(&v["roomInfo"]["tLiveInfo"]["tLiveStreamInfo"]["vStreamInfo"]["value"]) {
            let (base, stream) = (s(&line["sFlvUrl"]), s(&line["sStreamName"]));
            if base.is_empty() {
                continue;
            }
            let Ok(token) = self.cdn_token(&stream).await else { continue };
            let mut url = format!("{base}/{stream}.flv?{}&codec=264", build_anti_code(&stream, uid, &token, now_ms()));
            if bit_rate > 0 {
                url.push_str(&format!("&ratio={bit_rate}"));
            }
            urls.push(url);
        }
        if urls.is_empty() {
            return Err(anyhow!("虎牙没有可用的播放线路"));
        }
        Ok(PlayUrl { urls, headers: vec![("user-agent".into(), HYSDK_UA.into())] })
    }
}

/// 加入房间：WSUserInfo 包在 cmd=1 的外层里。sid 也传 topSid，与 Dart 一致。
fn join_packet(ayyuid: i64, top_sid: i64) -> Vec<u8> {
    let mut user = Writer::default();
    user.int(0, ayyuid).int(1, 1).string(2, "").string(3, "").int(4, top_sid).int(5, top_sid).int(6, 0).int(7, 0);
    let mut cmd = Writer::default();
    cmd.int(0, 1).bytes(1, &user.0);
    cmd.0
}

pub fn parse_message(frame: &[u8]) -> Option<LiveMessage> {
    let top = tars::decode(frame);
    if top.int(0) != 7 {
        return None;
    }
    let push = tars::decode(top.bytes(1));
    let msg = tars::decode(push.bytes(2));
    match push.int(1) {
        1400 => {
            let c = msg.struct_(6).int(0);
            let color = if c <= 0 { (255, 255, 255) } else { ((c >> 16) as u8, (c >> 8) as u8, c as u8) };
            Some(LiveMessage::Chat { user: msg.struct_(0).string(2), text: msg.string(3), color })
        }
        8006 => Some(LiveMessage::Online(msg.int(0))),
        _ => None,
    }
}

/// shortcut: 不重连，与 B站/斗鱼一致；断线多了再加。
pub async fn run_danmaku(ayyuid: i64, top_sid: i64, tx: UnboundedSender<LiveMessage>) -> Result<()> {
    let (ws, _) = tokio_tungstenite::connect_async("wss://cdnws.api.huya.com").await?;
    let (mut w, mut r) = ws.split();
    w.send(Message::Binary(join_packet(ayyuid, top_sid))).await?;
    let heartbeat = base64::engine::general_purpose::STANDARD.decode("ABQdAAwsNgBM")?;
    let mut beat = tokio::time::interval(Duration::from_secs(60));
    loop {
        tokio::select! {
            _ = beat.tick() => w.send(Message::Binary(heartbeat.clone())).await?,
            m = r.next() => match m {
                Some(Ok(Message::Binary(b))) => {
                    if let Some(m) = parse_message(&b) {
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
    fn rotl64_rotates_low_word_only() {
        assert_eq!(rotl64(0x1_1234_5678), 0x1_3456_7812);
    }

    #[test]
    fn anti_code_recomputes_secret() {
        let fm = base64::engine::general_purpose::STANDARD.encode("PREFIX_rest");
        let anti = format!("wsSecret=old&wsTime=65000000&fm={}&ctype=huya_pc_exe&t=100&fs=bgct", urlenc(&fm));
        let out = build_anti_code("stream1", 1, &anti, 1000);
        let hash = md5_hex("1001|huya_pc_exe|100");
        let expect = md5_hex(&format!("PREFIX_{}_stream1_{hash}_65000000", rotl64(1)));
        assert_eq!(query_get(&out, "wsSecret"), Some(expect));
        assert_eq!(query_get(&out, "u"), Some(rotl64(1).to_string()));
        assert_eq!(query_get(&out, "fm"), Some(fm));
        assert_eq!(build_anti_code("s", 1, "no_fm=1", 0), "no_fm=1");
    }

    #[test]
    fn parses_chat_and_online() {
        let push = |uri: i64, msg: &Writer| {
            let mut p = Writer::default();
            p.int(0, 0).int(1, uri).bytes(2, &msg.0);
            let mut top = Writer::default();
            top.int(0, 7).bytes(1, &p.0);
            top.0
        };
        let (mut sender, mut fmt, mut chat) = (Writer::default(), Writer::default(), Writer::default());
        sender.int(0, 1).string(2, "小明");
        fmt.int(0, 0xFF0000);
        chat.struct_(0, &sender).string(3, "你好").struct_(6, &fmt);
        match parse_message(&push(1400, &chat)) {
            Some(LiveMessage::Chat { user, text, color, .. }) => assert_eq!((user.as_str(), text.as_str(), color), ("小明", "你好", (255, 0, 0))),
            m => panic!("{m:?}"),
        }
        let mut online = Writer::default();
        online.int(0, 12345);
        assert!(matches!(parse_message(&push(8006, &online)), Some(LiveMessage::Online(12345))));
        assert!(parse_message(&join_packet(1, 2)).is_none());
    }
}
