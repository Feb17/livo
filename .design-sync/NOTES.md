# design-sync 笔记（LIVO）

- livo 本身是 Rust + GPUI，没有 React 组件库。Claude Design 用的是 `design/`（`@livo/design`）：按 LIVO 设计系统（Artifacts 里的「LIVO 设计系统」）写的一层 React 组件，渲染设计系统的 `lv-` 类和令牌。它不是 Livo 应用的运行代码，改 App 界面时要手工对应到 GPUI。
- 来源：`design/src/tokens.json` 与 `design/src/styles/components.css` 原样取自设计系统的 `project/tokens.json` 与 `project/components/bundle.css`；`design/src/styles/extra.css` 是 React 组件需要的补充样式。设计系统改色或改样式后，先把这两份文件重新复制进来，再同步。
- 图标：`design/src/icons.ts` 由 lucide-static 1.63.0 的 45 个 SVG 生成（ISC）。新增图标先在设计系统的 Iconography 清单登记。
- Logo 几何写在 `design/src/core.tsx`，坐标取自 Figma LIVO Brand Kit 导出的 SVG（livo-mark / livo-lockup / livo-signal、app icon 512），不要手改。
- 构建：`cd design && npm install && npm run build`（esbuild + tsc，产物 `design/dist/`，不进仓库）。npm 会提示 esbuild 的 install script 被拦，不影响使用。
- 转换器：`node .ds-sync/package-build.mjs --config .design-sync/config.json --node-modules ./design/node_modules --entry ./design/dist/index.js --out ./ds-bundle`。`.ds-sync/` 需要 `npm i esbuild ts-morph @types/react playwright` 和 `npx playwright install chromium`。
- 预览外框：`provider` 用 `LivoProvider`（深色，16px 内边距）。宽内容（卡片网格、播放器、工具栏、横幅等 16 个）用 `cardMode: column`；`Window` 用 `single` + `1140x680`。
- 字体：界面字体是 macOS 系统字体（SF Pro、苹方、SF Mono），无法随包分发，已列入 `runtimeFontPrefixes`；品牌字体 Outfit / Noto Sans SC 通过 Google Fonts `@import` 在线加载（`[FONT_REMOTE]`，正常）。
- 预览踩过的坑：工具栏里的 `SearchField` 固定 240px，不要再套更窄的外框（会盖住刷新按钮）；`SegmentedControl` 已禁止换行和压缩；单独使用的 `Sidebar` 需要固定 220px 宽（已在样式里处理）。

## Known render warns

- 无。

## Re-sync risks

- `design/src/tokens.json`、`components.css` 是从设计系统手工复制的快照，设计系统改了不会自动同步过来。
- 平台封面、头像在预览里用占位渐变，没有真实图片；`Cover` / `Avatar` 的 `src` 路径没有在 Claude Design 里实测过。
- 品牌字体依赖 Google Fonts 联网加载；离线时回退到系统字体。
- 首次同步在 2026-10-28 完成，验证用的 Chromium 由 playwright 1.64.1 安装。
