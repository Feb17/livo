---
category: Settings
---

设置窗口（⌘, 打开）。

宽 600，不可调整大小；工具栏标签（通用、播放、弹幕、快捷键）切换分类，窗口标题随标签变化；里面放 SettingsGroup。修改立即生效，没有「保存」「确定」。

```tsx
<SettingsWindow tabs={[{icon:'settings',label:'通用'},{icon:'play',label:'播放'}]} active="通用">
  <SettingsGroup title="外观">…</SettingsGroup>
</SettingsWindow>
```
