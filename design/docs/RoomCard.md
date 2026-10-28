---
category: Content
---

直播间卡片。

推荐、分区、搜索结果里成网格排列（放在 RoomGrid 里）。传标题、主播名、平台、人气；未开播传 `live={false}`。人气按中文缩写（1.2万）。悬停时描边与标题变 accent。

```tsx
<RoomGrid>
  <RoomCard title="【原神】深渊满星挑战" anchor="夜猫子小鹿" platform="bilibili" viewers="1.2万" />
</RoomGrid>
```
