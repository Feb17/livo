---
category: Content
---

16:9 直播封面。

封面居中裁切铺满，不留黑边、不拉伸；四种状态：loaded / loading（色块）/ error（image-off 图标）/ offline（去色、45% 不透明、「未开播」）。只叠 LIVE 与人气两枚徽标，标题放封面下方。一般通过 RoomCard 使用。

```tsx
<Cover live viewers="1.2万" />
```
