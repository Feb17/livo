---
category: Window
---

统一工具栏（高 52）。

左侧后退 / 前进与页面标题，右侧只放当前页面的高频操作：平台筛选 SegmentedControl、SearchField（⌘F）、刷新 IconButton（⌘R）。

```tsx
<Toolbar title="推荐">
  <SearchField />
  <IconButton icon="rotate-cw" label="刷新" />
</Toolbar>
```
