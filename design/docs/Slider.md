---
category: Inputs
---

滑块：调连续值。

设置页传 `label` 与 `valueLabel`（带单位，例如「8 秒」「85%」）；播放器进度与音量用 `variant="media"`（白色半透明轨道，进度可传 `buffered`）。

```tsx
<Slider label="弹幕速度" valueLabel="8 秒" value={40} />
```
