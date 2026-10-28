import { Skeleton } from '@livo/design';

export const Cards = () => (
  <div style={{ width: 600, display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 16 }}>
    <Skeleton /><Skeleton /><Skeleton />
  </div>
);
export const Rows = () => (
  <div style={{ width: 420, display: 'flex', flexDirection: 'column', gap: 16 }}>
    <Skeleton variant="row" /><Skeleton variant="row" />
  </div>
);
