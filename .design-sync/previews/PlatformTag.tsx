import { PlatformTag } from '@livo/design';

export const Platforms = () => (
  <div style={{ display: 'flex', gap: 10 }}>
    <PlatformTag platform="bilibili" />
    <PlatformTag platform="douyu" />
    <PlatformTag platform="huya" />
  </div>
);
