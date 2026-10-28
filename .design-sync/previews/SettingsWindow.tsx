import { SettingsWindow, SettingsGroup, SettingRow, SegmentedControl, PopupButton, Toggle, Button } from '@livo/design';

export const General = () => (
  <SettingsWindow
    active="通用"
    tabs={[{ icon: 'settings', label: '通用' }, { icon: 'play', label: '播放' }, { icon: 'message-square-text', label: '弹幕' }, { icon: 'keyboard', label: '快捷键' }]}
  >
    <SettingsGroup title="外观">
      <SettingRow title="主题" control={<SegmentedControl options={['跟随系统', '深色', '浅色']} />} />
      <SettingRow title="启动时打开" control={<PopupButton value="推荐" />} />
    </SettingsGroup>
    <SettingsGroup title="播放">
      <SettingRow title="默认清晰度" description="网络较差时自动降一档" control={<PopupButton value="原画" />} />
      <SettingRow title="低延迟模式" description="减少缓冲，网络不稳时可能卡顿" control={<Toggle checked label="低延迟模式" />} />
    </SettingsGroup>
    <SettingsGroup title="数据">
      <SettingRow title="观看历史" description="128 条记录" control={<Button>清空历史…</Button>} />
    </SettingsGroup>
  </SettingsWindow>
);
