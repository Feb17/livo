import { Cover } from '@livo/design';

export const States = () => (
  <div style={{ width: 800, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 12 }}>
    <Cover live viewers="1.2万" />
    <Cover state="loading" />
    <Cover state="error" />
    <Cover state="offline" />
  </div>
);
