---
category: Inputs
---

分段控件：2–4 个互斥选项，立即生效。

用于主题、默认清晰度、工具栏的平台筛选（全部 / 哔哩哔哩 / 斗鱼 / 虎牙）。超过 4 个选项改用 PopupButton + Menu。

```tsx
<SegmentedControl options={['全部', '哔哩哔哩', '斗鱼', '虎牙']} value="全部" />
```
