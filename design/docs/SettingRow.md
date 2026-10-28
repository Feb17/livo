---
category: Settings
---

设置行：左侧名称与说明，右侧控件。

`control` 放 Toggle、SegmentedControl、PopupButton、Button 或 KeybindingField；破坏性操作按钮文字带省略号（「清空历史…」）并弹 Dialog 确认。只能放在 SettingsGroup 里。

```tsx
<SettingRow title="默认清晰度" description="网络较差时自动降一档" control={<PopupButton value="原画" />} />
```
