---
category: Feedback
---

macOS 警告框，确认不可撤销的操作。

标题是问句并说清对象（「清空观看历史？」），说明一句范围与能否撤销，主按钮动词与标题对应；破坏性操作传 `destructive`。能撤销的操作不要弹窗，直接做并给 Toast。

```tsx
<Dialog title="清空观看历史？" message="将移除全部 128 条记录，此操作无法撤销。" confirmLabel="清空历史" destructive />
```
