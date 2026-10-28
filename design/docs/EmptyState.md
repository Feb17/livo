---
category: Feedback
---

空状态占位。

列表为空、搜索无结果、断网时使用：图标 + 一句现状 + 一句下一步 + 可选按钮。搜索无结果时在标题里回显关键词。不放插画和 emoji。

```tsx
<EmptyState icon="heart" title="还没有关注的主播" description="进入直播间点「关注」，开播后会排在这里。" action={<Button variant="primary">去看推荐</Button>} />
```
