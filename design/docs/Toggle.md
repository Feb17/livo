---
category: Inputs
---

开关：切换一个立即生效的设置。

放在 SettingRow 的 `control` 里；`label` 写设置名。需要点「保存」才生效的设置不要用开关。

```tsx
<SettingRow title="低延迟模式" control={<Toggle checked label="低延迟模式" />} />
```
