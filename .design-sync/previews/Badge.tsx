import { Badge } from '@livo/design';

export const Tones = () => (
  <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
    <Badge tone="live">LIVE</Badge>
    <Badge tone="quality">1080P 原画</Badge>
    <Badge tone="neutral">未开播</Badge>
    <Badge tone="media" icon="users">1.2万</Badge>
  </div>
);
