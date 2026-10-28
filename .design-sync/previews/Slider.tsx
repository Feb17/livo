import { Slider, Icon } from '@livo/design';

export const Settings = () => (
  <div style={{ width: 420, display: 'flex', flexDirection: 'column', gap: 18 }}>
    <Slider label="弹幕速度" valueLabel="8 秒" value={40} />
    <Slider label="弹幕不透明度" valueLabel="85%" value={85} />
  </div>
);
export const Volume = () => (
  <div style={{ width: 240, display: 'flex', gap: 8, alignItems: 'center', color: 'var(--text-muted)' }}>
    <Icon name="volume-1" />
    <Slider value={60} />
    <Icon name="volume-2" />
  </div>
);
