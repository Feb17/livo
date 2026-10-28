import { Avatar } from '@livo/design';

export const Sizes = () => (
  <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
    <Avatar name="夜猫子小鹿" size="sm" />
    <Avatar name="夜猫子小鹿" />
    <Avatar name="夜猫子小鹿" size="lg" />
    <Avatar name="夜猫子小鹿" size="xl" />
  </div>
);
export const LiveAndFallback = () => (
  <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
    <Avatar name="唱歌的橙子" size="lg" live />
    <Avatar size="lg" />
  </div>
);
