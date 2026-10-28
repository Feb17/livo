# LIVO 使用约定

LIVO 是 macOS 直播聚合应用（哔哩哔哩、斗鱼、虎牙）。界面文字一律中文，短句，按钮动词开头（「关注」「进入直播间」），中英文与数字之间留半角空格，人气按中文缩写（1.2万）。不用 emoji。

## 1. 根容器（必须）

所有页面都包在 `LivoProvider` 里。它提供 `data-theme`、系统字体（SF Pro + 苹方）、13px 正文和页面背景；不包时正文字体和背景都不对。深色是默认主题，浅色传 `theme="light"`。

```tsx
<LivoProvider theme="dark">
  <Window sidebar={…} toolbar={…}>…</Window>
</LivoProvider>
```

## 2. 样式写法

优先用组件，不要自己重画按钮、卡片、菜单、开关。自己写的布局胶水只用设计令牌（CSS 变量），不写死色值：

- 颜色：`var(--bg)` 页面底，`var(--sidebar)`，`var(--surface)` 卡片，`var(--surface-raised)` 弹层，`var(--hover)`，`var(--border)` 分隔线，`var(--border-strong)` 控件描边，`var(--text)` / `var(--text-muted)` / `var(--text-subtle)` 文字，`var(--accent)` 强调文字，`var(--accent-fill)` 紫色填充（上面放 `var(--on-accent)`），`var(--accent-soft)` 选中底，`var(--live)` 直播中青色，`var(--danger)` / `var(--warning)` / `var(--success)`。
- 间距：`var(--space-1)` 4 · `--space-2` 8 · `--space-3` 12 · `--space-4` 16 · `--space-6` 24 · `--space-8` 32。圆角：`--radius-xs` 4 · `--radius-sm` 6 · `--radius-md` 10 · `--radius-lg` 14 · `--radius-full`。
- 文字样式类：`lv-text-large-title`、`lv-text-title-1`、`lv-text-title-2`、`lv-text-title-3`、`lv-text-headline`、`lv-text-body`、`lv-text-callout`、`lv-text-caption`；品牌标题（官网、海报）用 `lv-text-display` / `lv-text-display-xl`。
- 品牌色比例：中性底约 60%，文字与内容 30%，紫色只给一个主按钮、选中态和进度，青色只表示直播中。平台色只出现在 `PlatformTag` / `PlatformDot`。
- 播放器区域（`Player`）永远深色，不跟随主题。

## 3. 页面结构（macOS）

- 主窗口：`Window` = `Sidebar`（发现 / 平台 / 我的）+ `Toolbar`（后退前进、标题、`SegmentedControl` 平台筛选、`SearchField`、刷新 `IconButton`）+ 内容区。内容区用 `SectionHeader` + `RoomGrid` 里的 `RoomCard`。
- 直播间：`Window padded={false}`，内容区横排：左列 `Player fill`（内含 `DanmakuLine`、`PlayerControls`）+ `RoomInfo`，右侧 `ChatPanel`（`ChatMessage`、`SuperChat`）。
- 我的关注 / 搜索主播：`AnchorList` + `AnchorRow`，在播在前。
- 设置：`SettingsWindow`（⌘, 打开，工具栏标签）+ `SettingsGroup` + `SettingRow`，控件用 `Toggle`、`SegmentedControl`、`PopupButton`、`KeybindingField`。没有「保存」按钮。
- 反馈：可撤销操作给 `Toast`；不可撤销操作弹 `Dialog`；持续问题用 `Banner`；空列表用 `EmptyState`；加载用 `Skeleton`。
- 图标只用 `Icon` 的清单名（Lucide），图标按钮必须有 `label`。标识只用 `Logo`，不要用文字打出「LIVO」。

## 4. 先读这些

- 样式：`styles.css` 及其引入的 `_ds_bundle.css`（全部令牌与 `lv-` 类）。
- 每个组件的 `.prompt.md`（用法与示例）和 `.d.ts`（属性）。

## 5. 示例

```tsx
<LivoProvider>
  <Window
    sidebar={<Sidebar><SidebarSection title="发现" /><SidebarItem icon="compass" active>推荐</SidebarItem><SidebarItem icon="heart" count="3 在播">关注</SidebarItem></Sidebar>}
    toolbar={<Toolbar title="推荐"><SearchField /><IconButton icon="rotate-cw" label="刷新" /></Toolbar>}
  >
    <SectionHeader title="正在直播" note="按人气排序" />
    <RoomGrid>
      <RoomCard title="【原神】深渊满星挑战" anchor="夜猫子小鹿" platform="bilibili" viewers="1.2万" />
    </RoomGrid>
    <div style={{ marginTop: 'var(--space-6)', display: 'flex', gap: 'var(--space-3)' }}>
      <Button variant="primary" icon="plus">关注</Button>
      <Button>复制链接</Button>
    </div>
  </Window>
</LivoProvider>
```
