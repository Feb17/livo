---
category: Player
---

悬浮在画面底部的播放控制条。

第一行时移进度；第二行播放/暂停、LIVE（或时移偏移 + 回到直播）、音量，右侧清晰度、线路、弹幕开关、弹幕设置、画中画、全屏。时移状态传 `live={false}` 与 `offset`。只能放在 Player 里。

```tsx
<PlayerControls live={false} offset="-00:42" />
```
