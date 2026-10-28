import { PlatformDot } from '@livo/design';

export const Legend = () => (
  <div style={{ display: 'flex', gap: 16, alignItems: 'center', fontSize: 12, color: 'var(--text-muted)' }}>
    <span style={{ display: 'inline-flex', gap: 6, alignItems: 'center' }}><PlatformDot platform="bilibili" />哔哩哔哩</span>
    <span style={{ display: 'inline-flex', gap: 6, alignItems: 'center' }}><PlatformDot platform="douyu" />斗鱼</span>
    <span style={{ display: 'inline-flex', gap: 6, alignItems: 'center' }}><PlatformDot platform="huya" />虎牙</span>
  </div>
);
