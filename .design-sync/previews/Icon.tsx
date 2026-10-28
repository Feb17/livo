import { Icon } from '@livo/design';

const names = ['compass', 'layout-grid', 'heart', 'history', 'search', 'settings', 'play', 'pause', 'volume-2', 'maximize', 'picture-in-picture-2', 'message-square-text', 'gauge', 'server', 'rotate-cw', 'copy', 'users', 'circle-alert', 'wifi-off', 'image-off'] as const;

export const Set = () => (
  <div style={{ display: 'grid', gridTemplateColumns: 'repeat(10, 32px)', gap: 12, color: 'var(--text)' }}>
    {names.map((n) => <Icon key={n} name={n} size={20} />)}
  </div>
);

export const SizesAndStates = () => (
  <div style={{ display: 'flex', gap: 20, alignItems: 'center' }}>
    <span style={{ color: 'var(--text-muted)' }}><Icon name="heart" /></span>
    <span style={{ color: 'var(--text-muted)' }}><Icon name="heart" size={20} /></span>
    <span style={{ color: 'var(--text)' }}><Icon name="heart" size={20} /></span>
    <span style={{ color: 'var(--accent)' }}><Icon name="heart" size={20} /></span>
    <span style={{ color: 'var(--danger)' }}><Icon name="circle-alert" size={20} /></span>
  </div>
);
