---
category: Player
---

直播间右侧的弹幕栏（宽 320）。

里面放 ChatMessage 与 SuperChat，消息从底部向上排；头部显示连接状态（实时 / 重连中）。

```tsx
<ChatPanel height={420}>
  <ChatMessage variant="system">小林 进入直播间</ChatMessage>
  <ChatMessage user="阿北">前方高能</ChatMessage>
</ChatPanel>
```
