---
category: Feedback
---

窗口内提示条。

放在内容区顶部，不遮挡画面。`error` 错误、`warning` 警告（断流重连中）、`info` 提示（时移中）。标题一句说发生了什么，`description` 说原因或下一步，可带一个按钮（「重试」）。不怪用户。

```tsx
<Banner tone="error" title="无法加载直播间" description="平台接口返回错误，稍后再试或切换线路。" action={<Button>重试</Button>} />
```
