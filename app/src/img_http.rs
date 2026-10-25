//! 给 gpui 的 img(url) 用的 HTTP 客户端：GPUI 负责解码和缓存，这里只管把字节拉回来。
use gpui::http_client::{http::HeaderValue, AsyncBody, HttpClient, Request, Response, Url};
use std::future::Future;
use std::pin::Pin;

pub struct ImageClient(reqwest::Client);

impl ImageClient {
    pub fn new() -> Self {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
        Self(reqwest::Client::builder().user_agent(ua).build().expect("http client"))
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
                    let r = client.get(&uri).send().await?;
                    anyhow::Ok((r.status().as_u16(), r.bytes().await?.to_vec()))
                })
                .await??;
            Ok(Response::builder().status(status).body(AsyncBody::from(body))?)
        })
    }
}
