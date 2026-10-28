---
category: Status
---

直播状态：圆点 + 文字。

`live` 为青色「直播中」，否则灰色「未开播」；`label` 可改为「实时」「重连中」。颜色只是辅助，永远带文字。

```tsx
<LiveStatus />
<LiveStatus live={false} />
```
