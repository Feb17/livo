---
category: Menus
---

菜单面板：弹出菜单与右键菜单。

里面放 MenuLabel、MenuItem、MenuSeparator。卡片、主播行、历史行都要有右键菜单，首项是默认操作（进入直播间），破坏性操作放最后并用分隔线隔开。单选菜单（清晰度、线路）的项设 `selectable`。

```tsx
<Menu>
  <MenuItem icon="play">进入直播间</MenuItem>
  <MenuSeparator />
  <MenuItem icon="copy" shortcut="⌘C">复制链接</MenuItem>
</Menu>
```
