import { Tooltip, IconButton } from '@livo/design';

export const WithShortcut = () => (
  <div style={{ display: 'flex', gap: 24 }}>
    {[['画中画', '⌥⌘P', 'picture-in-picture-2'], ['关闭弹幕', 'D', 'message-square-text'], ['刷新', '⌘R', 'rotate-cw']].map(([l, s, i]) => (
      <div key={l} style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6 }}>
        <Tooltip label={l} shortcut={s} />
        <IconButton icon={i as 'rotate-cw'} label={l} />
      </div>
    ))}
  </div>
);
