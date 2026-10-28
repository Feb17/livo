---
category: Window
---

侧栏（macOS 源列表），宽 220。

顶部自带红绿灯；分组「发现（推荐、分区）/ 平台（哔哩哔哩、斗鱼、虎牙）/ 我的（关注、历史）」。设置不放侧栏，走 ⌘, 打开设置窗口。

```tsx
<Sidebar>
  <SidebarSection title="发现" />
  <SidebarItem icon="compass" active>推荐</SidebarItem>
</Sidebar>
```
