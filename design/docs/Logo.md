---
category: Foundation
---

LIVO 标识：符号、组合标识、信号弧、App 图标。

页头 / 关于页用 `variant="lockup"`；头像、小位置用 `mark`；16–23px 用 `mark-small`；深色底的组合标识用 `tone="reverse"`；品牌紫底只用 `tone="white"`。只等比缩放，不改色、不旋转、不加阴影，不要把符号和组合标识并排，不要用文字打出「LIVO」代替。

```tsx
<Logo variant="lockup" height={28} />
<Logo variant="mark" height={24} />
```
