---
category: Player
---

播放器画面。

永远深色、不跟随主题；里面放 DanmakuLine 与 PlayerControls。直播间页面传 `fill` 撑满剩余空间（画面按比例居中）；全屏时传 `title` 显示直播间标题。

```tsx
<Player>
  <DanmakuLine top={20} left={50}>前方高能</DanmakuLine>
  <PlayerControls />
</Player>
```
