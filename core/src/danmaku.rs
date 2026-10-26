use crate::{DanmakuArgs, LiveMessage};
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::io::Read;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

#[derive(Clone, Debug, Default)]
pub struct BiliDanmakuArgs {
    pub room_id: i64,
    pub uid: i64,
    pub token: String,
    pub server_host: String,
    pub buvid: String,
    pub cookie: String,
}

/// 16 字节头：总长、头长(16)、协议版本、操作码、序号(1)，均为大端。
pub fn encode(body: &str, op: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + body.len());
    out.extend((16 + body.len() as u32).to_be_bytes());
    out.extend(16u16.to_be_bytes());
    out.extend(0u16.to_be_bytes());
    out.extend(op.to_be_bytes());
    out.extend(1u32.to_be_bytes());
    out.extend(body.as_bytes());
    out
}

/// 解一个 WebSocket 帧里的所有包；压缩包（zlib/brotli）递归展开。
pub fn decode(data: &[u8], out: &mut Vec<LiveMessage>) {
    let mut pos = 0;
    while pos + 16 <= data.len() {
        let len = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        if len < 16 || pos + len > data.len() {
            break;
        }
        let ver = u16::from_be_bytes(data[pos + 6..pos + 8].try_into().unwrap());
        let op = u32::from_be_bytes(data[pos + 8..pos + 12].try_into().unwrap());
        let body = &data[pos + 16..pos + len];
        pos += len;
        match (op, ver) {
            (3, _) if body.len() >= 4 => out.push(LiveMessage::Online(u32::from_be_bytes(body[..4].try_into().unwrap()) as i64)),
            (5, 2) => {
                let mut buf = vec![];
                if flate2::read::ZlibDecoder::new(body).read_to_end(&mut buf).is_ok() {
                    decode(&buf, out);
                }
            }
            (5, 3) => {
                let mut buf = vec![];
                if brotli::Decompressor::new(body, 4096).read_to_end(&mut buf).is_ok() {
                    decode(&buf, out);
                }
            }
            (5, _) => {
                if let Ok(v) = serde_json::from_slice::<Value>(body) {
                    if let Some(m) = parse_message(&v) {
                        out.push(m);
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn parse_message(v: &Value) -> Option<LiveMessage> {
    let cmd = v["cmd"].as_str()?;
    if cmd.contains("DANMU_MSG") {
        let info = &v["info"];
        let user = info[2][1].as_str()?.to_string();
        let text = info[1].as_str()?.to_string();
        let c = info[0][3].as_u64().unwrap_or(0) as u32;
        let color = if c == 0 { (255, 255, 255) } else { ((c >> 16) as u8, (c >> 8) as u8, c as u8) };
        Some(LiveMessage::Chat { user, text, color })
    } else if cmd == "SUPER_CHAT_MESSAGE" {
        let d = &v["data"];
        Some(LiveMessage::SuperChat {
            user: d["user_info"]["uname"].as_str()?.to_string(),
            text: d["message"].as_str().unwrap_or("").to_string(),
            price: d["price"].as_i64().unwrap_or(0),
        })
    } else {
        None
    }
}

/// 连接对应平台的弹幕服务器，消息通过 `tx` 送出；`tx` 关闭或连接断开时返回。
/// shortcut: 不做自动重连，断线由调用方决定是否重新 run，需要时再加。
pub async fn run(args: DanmakuArgs, tx: UnboundedSender<LiveMessage>) -> Result<()> {
    match args {
        DanmakuArgs::BiliBili(a) => run_bili(a, tx).await,
        DanmakuArgs::Douyu(rid) => crate::douyu::run_danmaku(rid, tx).await,
        DanmakuArgs::Huya { ayyuid, top_sid, .. } => crate::huya::run_danmaku(ayyuid, top_sid, tx).await,
        DanmakuArgs::None => Err(anyhow!("该平台暂不支持弹幕")),
    }
}

async fn run_bili(args: BiliDanmakuArgs, tx: UnboundedSender<LiveMessage>) -> Result<()> {
    if args.token.is_empty() {
        return Err(anyhow!("弹幕信息缺失，已跳过弹幕连接"));
    }
    let mut req = format!("wss://{}/sub", args.server_host).into_client_request()?;
    if !args.cookie.is_empty() {
        req.headers_mut().insert("cookie", args.cookie.parse()?);
    }
    let (ws, _) = tokio_tungstenite::connect_async(req).await?;
    let (mut w, mut r) = ws.split();
    let join = json!({"uid": args.uid, "roomid": args.room_id, "protover": 3, "buvid": args.buvid, "platform": "web", "type": 2, "key": args.token});
    w.send(Message::Binary(encode(&join.to_string(), 7))).await?;
    let mut beat = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = beat.tick() => w.send(Message::Binary(encode("", 2))).await?,
            m = r.next() => match m {
                Some(Ok(Message::Binary(b))) => {
                    let mut msgs = vec![];
                    decode(&b, &mut msgs);
                    for m in msgs { if tx.send(m).is_err() { return Ok(()); } }
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
    fn roundtrip_chat_packet() {
        let body = r#"{"cmd":"DANMU_MSG","info":[[0,1,25,16777215,0,0,0,"x",0,0,0,"",0,{"url":"http://e/1.png"}],"你好",[1,"小明"]]}"#;
        let mut pkt = encode(body, 5);
        pkt.extend(encode("", 5)); // 空包不应中断解析
        let mut out = vec![];
        decode(&pkt, &mut out);
        match &out[0] {
            LiveMessage::Chat { user, text, color } => assert_eq!((user.as_str(), text.as_str(), *color), ("小明", "你好", (255, 255, 255))),
            m => panic!("{m:?}"),
        }
    }

    #[test]
    fn online_packet() {
        let mut p = encode("", 3);
        p.truncate(16);
        p.extend(1234u32.to_be_bytes());
        let l = (p.len() as u32).to_be_bytes();
        p[..4].copy_from_slice(&l);
        let mut out = vec![];
        decode(&p, &mut out);
        assert!(matches!(out[0], LiveMessage::Online(1234)));
    }
}
