import { IconButton } from '@livo/design';

export const Toolbar = () => (
  <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
    <IconButton icon="chevron-left" label="后退" />
    <IconButton icon="chevron-right" label="前进" disabled />
    <IconButton icon="rotate-cw" label="刷新" />
    <IconButton icon="copy" label="复制链接" />
    <IconButton icon="ellipsis" label="更多" />
  </div>
);

export const Pressed = () => (
  <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
    <IconButton icon="message-square-text" label="关闭弹幕" pressed />
    <IconButton icon="message-square-off" label="打开弹幕" />
  </div>
);
