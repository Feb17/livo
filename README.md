<p align="center"><img width="128" src="app/assets/icon.png" alt="Livo"></p>
<h2 align="center">Livo（Rust + GPUI 版）</h2>

<p align="center">用 Rust 和 <a href="https://github.com/zed-industries/zed/tree/main/crates/gpui">GPUI</a> 重写的 <a href="https://github.com/xiaoyaocz/dart_simple_live">Simple Live</a>：轻量、高性能的聚合直播客户端。</p>

## 功能

- 平台：哔哩哔哩、斗鱼、虎牙（推荐、分类、搜索、直播间）
- 播放：内嵌 libmpv；macOS 走 VideoToolbox 硬解 + OpenGL→NV12 IOSurface 零拷贝显示，其他平台软件渲染
- 弹幕：聊天区 + 视频上的滚动弹幕（字号、透明度、速度、区域可调）
- 关注、观看记录、B站扫码登录、浅色/深色主题、快捷键自定义、定时关闭
- 中文输入法搜索

## 下载

见 [Releases](../../releases)：

- **macOS（Apple silicon）**：`Livo-macOS-arm64-*.dmg`。应用未经 Apple 公证，首次打开若提示“已损坏/无法验证”，执行 `xattr -cr "/Applications/Livo.app"` 后再打开。
- **Windows**：暂停维护，最后一个 Windows 版是 [v0.1.5](../../releases/tag/v0.1.5)。

## 从源码构建

需要 Rust（stable）和 libmpv：

```bash
# macOS
brew install mpv
cargo run --release -p livo

# 打包 macOS .app 和 dmg，产物在 dist/（会下载 media-kit 的精简版 libmpv 打进包里，不用 Homebrew 的）
scripts/bundle-macos.sh
```

接口自测（会访问各平台线上接口）：`cargo run --example smoke -p livo_core`

## 发版

先改好 [`app_version.json`](app_version.json)（`version` 版本号、`version_desc` 更新说明、`prerelease` 是否预发行）并提交，再推送同版本的标签（如 `git tag v0.1.2 && git push origin v0.1.2`），[release 工作流](.github/workflows/release.yml)会构建 macOS dmg，按这个文件创建 Release；标签和文件里的版本号不一致时直接失败。也可以在 Actions 页面手动运行，只产出构建产物。

## 致谢与许可证

平台接口、弹幕协议等逻辑移植自 [xiaoyaocz/dart_simple_live](https://github.com/xiaoyaocz/dart_simple_live)（及其社区维护分支），斗鱼签名与虎牙部分参考了 [hua0512/rust-srec](https://github.com/hua0512/rust-srec)。播放基于 [mpv](https://mpv.io)，内置 [media-kit/libmpv-darwin-build](https://github.com/media-kit/libmpv-darwin-build) 的精简版 libmpv。感谢原作者和所有上游开源项目。

本项目沿用原项目的 [GPL-3.0](LICENSE) 许可证。
