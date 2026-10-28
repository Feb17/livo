---
category: Window
---

macOS 主窗口框架。

所有页面都放在 Window 里：`sidebar` 传 Sidebar，`toolbar` 传 Toolbar，内容放 children。直播间页面传 `padded={false}`，内容区横向放 Player 列与 ChatPanel。窗口最小 960×600。

```tsx
<Window sidebar={<Sidebar>…</Sidebar>} toolbar={<Toolbar title="推荐" />}>
  <SectionHeader title="正在直播" />
  <RoomGrid>…</RoomGrid>
</Window>
```
