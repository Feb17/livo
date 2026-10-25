<p align="center"><img width="128" src="app/assets/icon.png" alt="Simple Live"></p>
<h2 align="center">Simple Live（Rust + GPUI 版）</h2>

<p align="center">用 Rust 和 <a href="https://github.com/zed-industries/zed/tree/main/crates/gpui">GPUI</a> 重写的 <a href="https://github.com/xiaoyaocz/dart_simple_live">Simple Live</a>：轻量、高性能的聚合直播客户端。</p>

## 功能

- 平台：哔哩哔哩、斗鱼、虎牙（推荐、分类、搜索、直播间）
- 播放：内嵌 libmpv；macOS 走 VideoToolbox 硬解 + OpenGL→NV12 IOSurface 零拷贝显示，其他平台软件渲染
- 弹幕：聊天区 + 视频上的滚动弹幕（字号、透明度、速度、区域可调）
- 关注、观看记录、B站扫码登录、浅色/深色主题、快捷键自定义、定时关闭
- 中文输入法搜索

## 下载

见 [Releases](../../releases)：

- **macOS（Apple silicon）**：`SimpleLive-macOS-arm64-*.dmg`。应用未经 Apple 公证，首次打开若提示“已损坏/无法验证”，执行 `xattr -cr "/Applications/Simple Live.app"` 后再打开。
- **Windows（x64）**：`SimpleLive-Windows-x64-*-portable.zip`，解压后运行 `SimpleLive.exe`。

## 从源码构建

需要 Rust（stable）和 libmpv：

```bash
# macOS
brew install mpv
cargo run --release -p simple_live_gpui

# 打包 macOS .app 和 dmg（另需 brew install dylibbundler），产物在 dist/
scripts/bundle-macos.sh
```

Windows 需要 [libmpv 开发包](https://github.com/shinchiro/mpv-winbuild-cmake/releases)（`mpv-dev-x86_64-*.7z`）：解压后把 `libmpv.dll.a` 复制为 `mpv.lib`，设置环境变量 `MPV_LIB_DIR` 指向该目录再 `cargo build --release`，运行时把 `libmpv-2.dll` 放在 exe 旁边。

接口自测（会访问各平台线上接口）：`cargo run --example smoke -p simple_live_core`

## 发版

推送 `v*` 标签（如 `git tag v0.1.0 && git push origin v0.1.0`）会触发 [release 工作流](.github/workflows/release.yml)，构建 Windows portable 包和 macOS dmg 并发布到 Releases；也可以在 Actions 页面手动运行，只产出构建产物。

## 致谢与许可证

平台接口、弹幕协议等逻辑移植自 [xiaoyaocz/dart_simple_live](https://github.com/xiaoyaocz/dart_simple_live)（及其社区维护分支），斗鱼签名与虎牙部分参考了 [hua0512/rust-srec](https://github.com/hua0512/rust-srec)。感谢原作者和所有上游开源项目。

本项目沿用原项目的 [GPL-3.0](LICENSE) 许可证。
