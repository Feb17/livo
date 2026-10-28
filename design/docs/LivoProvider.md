---
category: Foundation
---

所有 LIVO 组件的根容器：设置主题（深色默认 / 浅色）、系统字体、13px 正文与页面背景。

把整个页面包在 `<LivoProvider>` 里；浅色界面传 `theme="light"`。不包时令牌仍生效，但正文字体、字号与背景不对。

```tsx
<LivoProvider theme="dark">
  <Window sidebar={…} toolbar={…}>…</Window>
</LivoProvider>
```
