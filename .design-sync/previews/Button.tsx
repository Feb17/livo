import { Button } from '@livo/design';

export const Variants = () => (
  <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
    <Button variant="primary" icon="plus">关注</Button>
    <Button icon="copy">复制链接</Button>
    <Button variant="ghost">取消</Button>
    <Button variant="danger" icon="trash-2">清空历史</Button>
  </div>
);

export const Large = () => (
  <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
    <Button variant="primary" size="lg">进入直播间</Button>
    <Button size="lg">稍后再看</Button>
  </div>
);

export const Disabled = () => (
  <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
    <Button icon="check" disabled>已关注</Button>
    <Button variant="primary" disabled>进入直播间</Button>
  </div>
);
