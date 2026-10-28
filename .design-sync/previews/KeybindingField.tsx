import { KeybindingField, SettingsGroup, SettingRow } from '@livo/design';

export const States = () => (
  <div style={{ width: 460 }}>
    <SettingsGroup title="播放器">
      <SettingRow title="播放 / 暂停" control={<KeybindingField keys={['Space']} />} />
      <SettingRow title="全屏" control={<KeybindingField recording />} />
      <SettingRow title="弹幕开关" control={<KeybindingField keys={['⌘', 'F']} conflict="与「搜索」冲突" />} />
    </SettingsGroup>
  </div>
);
