import { Logo } from '@livo/design';

export const Marks = () => (
  <div style={{ display: 'flex', gap: 28, alignItems: 'flex-end' }}>
    <Logo variant="mark" height={56} />
    <Logo variant="mark" height={24} />
    <Logo variant="mark-small" height={18} />
    <Logo variant="signal" height={40} />
  </div>
);

export const Lockups = () => (
  <div style={{ display: 'flex', gap: 24, alignItems: 'center' }}>
    <div style={{ background: '#F8F9FD', padding: 16, borderRadius: 10 }}><Logo variant="lockup" height={40} /></div>
    <div style={{ background: '#1F2234', padding: 16, borderRadius: 10 }}><Logo variant="lockup" tone="reverse" height={40} /></div>
    <div style={{ background: '#6E58F5', padding: 16, borderRadius: 10 }}><Logo variant="lockup" tone="white" height={40} /></div>
  </div>
);

export const AppIcons = () => (
  <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
    <Logo variant="app-icon" height={96} />
    <Logo variant="app-icon" appearance="dark" height={96} />
    <Logo variant="app-icon" height={48} />
  </div>
);
