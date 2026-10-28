import { Toggle, SettingsGroup, SettingRow } from '@livo/design';

export const States = () => (
  <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
    <Toggle checked label="低延迟模式" />
    <Toggle label="显示弹幕" />
  </div>
);
export const InSettings = () => (
  <div style={{ width: 460 }}>
    <SettingsGroup title="播放">
      <SettingRow title="低延迟模式" description="减少缓冲，网络不稳时可能卡顿" control={<Toggle checked label="低延迟模式" />} />
      <SettingRow title="进入直播间时显示弹幕" control={<Toggle label="显示弹幕" />} />
    </SettingsGroup>
  </div>
);
