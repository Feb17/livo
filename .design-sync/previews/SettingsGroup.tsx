import { SettingsGroup, SettingRow, Toggle, PopupButton } from '@livo/design';

export const Playback = () => (
  <div style={{ width: 480 }}>
    <SettingsGroup title="播放">
      <SettingRow title="默认清晰度" description="网络较差时自动降一档" control={<PopupButton value="原画" />} />
      <SettingRow title="低延迟模式" description="减少缓冲，网络不稳时可能卡顿" control={<Toggle checked label="低延迟模式" />} />
    </SettingsGroup>
  </div>
);
