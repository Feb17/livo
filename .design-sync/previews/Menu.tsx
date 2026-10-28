import { Menu, MenuItem, MenuLabel, MenuSeparator, PopupButton } from '@livo/design';

export const Quality = () => (
  <div style={{ display: 'flex', flexDirection: 'column', gap: 6, alignItems: 'flex-start' }}>
    <PopupButton value="原画" />
    <Menu ariaLabel="清晰度">
      <MenuLabel>清晰度</MenuLabel>
      <MenuItem selectable checked detail="1080P">原画</MenuItem>
      <MenuItem selectable highlighted detail="720P">超清</MenuItem>
      <MenuItem selectable>流畅</MenuItem>
      <MenuSeparator />
      <MenuLabel>线路</MenuLabel>
      <MenuItem selectable checked>线路 1</MenuItem>
      <MenuItem selectable>线路 2</MenuItem>
    </Menu>
  </div>
);
export const ContextMenu = () => (
  <Menu ariaLabel="直播间">
    <MenuItem icon="play">进入直播间</MenuItem>
    <MenuItem icon="plus">关注</MenuItem>
    <MenuSeparator />
    <MenuItem icon="copy" shortcut="⌘C">复制链接</MenuItem>
    <MenuItem icon="external-link">在浏览器中打开</MenuItem>
    <MenuSeparator />
    <MenuItem icon="trash-2" danger highlighted>从历史中移除</MenuItem>
  </Menu>
);
