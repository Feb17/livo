---
category: Actions
---

文字按钮，文字动词开头。

`primary` 主操作，一屏只有一个；`secondary` 次操作（默认）；`ghost` 取消与工具栏文字按钮；`danger` 只用于不可撤销的删除并配确认。默认 28px，搜索栏旁与空状态主操作用 `size="lg"`。可带 16px 前置图标。不要写「确定」「提交」。

```tsx
<Button variant="primary" icon="plus">关注</Button>
<Button>复制链接</Button>
```
