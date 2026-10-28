import { LivoProvider, Button, LiveStatus } from '@livo/design';

export const Dark = () => (
  <LivoProvider theme="dark" style={{ padding: 16, display: 'flex', gap: 12, alignItems: 'center' }}>
    <span className="lv-text-title-3">我的关注</span>
    <LiveStatus />
    <Button variant="primary" icon="plus">关注</Button>
  </LivoProvider>
);

export const Light = () => (
  <LivoProvider theme="light" style={{ padding: 16, display: 'flex', gap: 12, alignItems: 'center' }}>
    <span className="lv-text-title-3">我的关注</span>
    <LiveStatus />
    <Button variant="primary" icon="plus">关注</Button>
  </LivoProvider>
);
