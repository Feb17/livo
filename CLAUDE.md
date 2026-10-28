# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Livo is a Rust + GPUI rewrite of [dart_simple_live](https://github.com/xiaoyaocz/dart_simple_live): a desktop live-stream aggregator for Bilibili, Douyu and Huya. Platform APIs and danmaku protocols are ported from that project; comments like "同原项目" mean "same as the original Dart project", which is the reference when behavior is in doubt. Code comments, UI strings and commit messages are in Chinese.

Livo is a macOS app (Apple silicon) and follows Apple's Human Interface Guidelines for macOS: native menu bar, standard ⌘ shortcuts, single-window lifecycle. Windows support is paused (last Windows build: v0.1.5) — don't add Windows code paths.

## Working rules

- 不清楚怎么实现的功能，不要马上从零开始写：先去 GitHub 社区搜类似的开源项目（同类 GPUI/Rust 桌面应用、mpv 播放器、原项目 dart_simple_live 等），对比它们的思路，再动手。

## Commands

libmpv must be installed and linkable (`brew install mpv` on macOS; `app/build.rs` adds `/opt/homebrew/lib`, `/usr/local/lib` and `$MPV_LIB_DIR` to the link search path).

```bash
cargo run --release -p livo                 # run the app
cargo test -p livo_core                     # unit tests (only core has tests)
cargo test -p livo_core douyu::tests::name  # a single test
cargo run --example smoke -p livo_core      # end-to-end check against the live platform APIs (network)
scripts/bundle-macos.sh [version]           # build .app + .dmg into dist/, bundles media-kit's slim libmpv instead of Homebrew's
```

## Release

Edit `app_version.json` (`version`, `version_desc`, `prerelease`), commit, then push a matching tag `vX.Y.Z`. `.github/workflows/release.yml` fails if the tag and `app_version.json` disagree, builds the macOS dmg, signs it, and creates the Release from that file. Keep `app/Cargo.toml`'s version in step — untagged builds and `bundle-macos.sh` read it.

## Architecture

Two-crate workspace:

- **`core/` (`livo_core`)** — UI-free platform clients on tokio + reqwest. `lib.rs` defines the shared models (`RoomItem`, `RoomDetail`, `PlayUrl`, `LiveMessage`, `DanmakuArgs`, …) and `enum Site`, which dispatches to `bilibili`/`douyu`/`huya` through the `each!` macro instead of async trait objects. Adding a site method means adding it to every platform module and to `Site`. `Site::id()` is the persisted key — don't change it. `danmaku::run` connects to a platform's danmaku websocket and pushes `LiveMessage`s into an mpsc channel; it does not reconnect. `tars.rs` is the TARS codec Huya's danmaku protocol needs.
- **`app/` (`livo`)** — the GPUI desktop UI.
  - UI follows the LIVO design system (a Design System artifact on claude.ai): `theme.rs` is its generated token export (`DARK`/`LIGHT` palettes as 0xRRGGBBAA, spacing, radii, sizes) — don't hand-edit it, change the design system and re-export. `ui.rs` turns the tokens into GPUI: `pal()` and color helpers (`bg()`, `text()`, `accent()`, …), the type scale (`.t_body()`, `.t_title3()`, …), shadows, and shared controls (`button`, `icon_btn`, `badge`, `platform_tag`, `avatar`, `empty_state`, `skeleton`, `tip`). Don't write raw colors in views; the player area (video, controls, danmaku) always uses `theme::DARK`. Icons are the design system's Lucide 1.63.0 list in `icons.rs`.
  - `main.rs` holds almost all UI: the root `Livo` entity, sidebar/toolbar/pages, `Room`, `Video`, `Chat`, and the IME `EntityInputHandler` for search. `settings.rs` adds more `impl Livo` blocks for the settings/history pages.
  - `menus.rs`: the macOS menu bar. Every menu item is a GPUI action with its ⌘ binding; app-level ones are handled globally there, the rest via `.on_action` on `Livo`'s root div — room-only actions are registered only while in a room, so their items grey out elsewhere. The 窗口/帮助 menus are handed to AppKit via `objc` (GPUI only recognises an English "Window" menu). Closing the window quits the app.
  - Async model: core futures run on a global 2-thread tokio runtime (`rt()`), and results come back to the UI thread via `cx.spawn`. `Livo::fetch` wraps this and bumps `epoch`, so stale responses are dropped — use it for page loads. Dropping a `Room` aborts its danmaku task.
  - Playback: `mpv.rs` is hand-written libmpv FFI (no crate). On macOS, `mpv_gl.rs` renders via OpenGL and converts to NV12 IOSurface for `gpui::surface` (zero-copy; GPUI's surface only accepts 420f). Other platforms, or when GL fails, use software rendering. Gate macOS-only code with `#[cfg(target_os = "macos")]`.
  - `store.rs`: settings, follows and history in one JSON file, `~/Library/Application Support/Livo/settings.json`. A corrupt file is saved aside as `settings.json.bad` and replaced with defaults instead of blocking startup. New fields need `#[serde(default)]` so old files keep loading.
  - `img_http.rs`: the `HttpClient` behind GPUI's `img(url)`, with retries and downscaling of large covers (to 640px) to keep memory down.
