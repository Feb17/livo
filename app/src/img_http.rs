//! 给 gpui 的 img(url) 用的 HTTP 客户端：GPUI 负责解码和缓存，这里负责拉字节、重试，并把过大的图先缩小。
use gpui::http_client::{http::HeaderValue, AsyncBody, HttpClient, Request, Response, Url};
use std::future::Future;
use std::pin::Pin;

pub struct ImageClient(reqwest::Client);

/// 卡片最宽不到 2 × MIN_CARD，Retina 下 640px 足够清晰
const MAX_SIDE: u32 = 640;

/// 虎牙部分封面是 1080p/4K 原图，GPUI 解码后单张 8-32MB，内存、显存各占一份，所以先缩到卡片用得上的尺寸。
/// 不认识的格式或解码失败就原样交给 GPUI。
fn shrink(bytes: Vec<u8>) -> Vec<u8> {
    let reader = || image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format().ok();
    let small = reader()
        .and_then(|r| r.into_dimensions().ok())
        .filter(|&(w, h)| w > MAX_SIDE || h > MAX_SIDE)
        .and_then(|_| reader()?.decode().ok())
        .and_then(|img| {
            let mut out = std::io::Cursor::new(Vec::new());
            img.thumbnail(MAX_SIDE, MAX_SIDE).write_to(&mut out, image::ImageFormat::Png).ok()?;
            Some(out.into_inner())
        });
    small.unwrap_or(bytes)
}

impl ImageClient {
    pub fn new() -> Self {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
        // 卡住的请求会让图片永远停在加载中，所以给总时长设上限
        Self(reqwest::Client::builder().user_agent(ua).timeout(std::time::Duration::from_secs(15)).build().expect("http client"))
    }
}

impl HttpClient for ImageClient {
    fn type_name(&self) -> &'static str {
        "ImageClient"
    }

    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    /// shortcut: 只发 GET（img 只用到 GET），要给 GPUI 别的功能用再补方法和请求体。
    fn send(&self, req: Request<AsyncBody>) -> Pin<Box<dyn Future<Output = anyhow::Result<Response<AsyncBody>>> + Send>> {
        let (client, uri) = (self.0.clone(), req.uri().to_string());
        Box::pin(async move {
            // reqwest 需要 tokio，GPUI 的后台执行器不是，所以借用应用的 tokio 运行时
            let (status, body) = crate::rt()
                .spawn(async move {
                    // GPUI 会一直缓存失败结果直到重启，所以网络错误和 5xx 先重试一次
                    let get = || async { anyhow::Ok(client.get(&uri).send().await?.error_for_status()?.bytes().await?) };
                    match get().await {
                        Ok(b) => anyhow::Ok((200, b.to_vec())),
                        Err(e) if e.downcast_ref::<reqwest::Error>().and_then(|e| e.status()).is_some_and(|s| s.is_client_error()) => Err(e),
                        Err(_) => Ok((200, get().await?.to_vec())),
                    }
                })
                .await??;
            Ok(Response::builder().status(status).body(AsyncBody::from(shrink(body)))?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// 本地服务：按顺序对每个连接回一个状态码，返回 (地址, 收到的连接数)
    fn serve(codes: &'static [u16]) -> (String, std::thread::JoinHandle<usize>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}/a.png", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            for code in codes {
                let (mut s, _) = l.accept().unwrap();
                s.read(&mut [0; 1024]).unwrap();
                write!(s, "HTTP/1.1 {code} X\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok").unwrap();
            }
            codes.len()
        });
        (addr, h)
    }

    fn get(url: &str) -> anyhow::Result<Response<AsyncBody>> {
        let req = Request::get(url).body(AsyncBody::empty()).unwrap();
        crate::rt().block_on(ImageClient::new().send(req))
    }

    #[test]
    fn shrinks_only_oversized_images() {
        let png = |w, h| {
            let mut out = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(w, h).write_to(&mut out, image::ImageFormat::Png).unwrap();
            out.into_inner()
        };
        let small = png(338, 190);
        assert_eq!(shrink(small.clone()), small);
        let dims = image::load_from_memory(&shrink(png(3840, 2160))).unwrap();
        assert_eq!((dims.width(), dims.height()), (640, 360));
        assert_eq!(shrink(b"not an image".to_vec()), b"not an image");
    }

    #[test]
    fn retries_server_errors_once_but_not_404() {
        let (url, h) = serve(&[500, 200]);
        assert_eq!(get(&url).unwrap().status(), 200);
        assert_eq!(h.join().unwrap(), 2);
        let (url, h) = serve(&[404]);
        assert!(get(&url).is_err());
        assert_eq!(h.join().unwrap(), 1);
    }
}
