import { SettingsGroup, SettingRow, SegmentedControl, Button } from '@livo/design';

export const Controls = () => (
  <div style={{ width: 520 }}>
    <SettingsGroup>
      <SettingRow title="主题" control={<SegmentedControl options={['跟随系统', '深色', '浅色']} />} />
      <SettingRow title="观看历史" description="128 条记录" control={<Button>清空历史…</Button>} />
    </SettingsGroup>
  </div>
);
