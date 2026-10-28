import { LiveStatus } from '@livo/design';

export const States = () => (
  <div style={{ display: 'flex', gap: 20 }}>
    <LiveStatus />
    <LiveStatus live={false} />
    <LiveStatus label="实时" />
    <LiveStatus live={false} label="重连中" />
  </div>
);
