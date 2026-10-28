import { Menu, MenuItem } from '@livo/design';

export const Kinds = () => (
  <Menu>
    <MenuItem icon="copy" shortcut="⌘C">复制链接</MenuItem>
    <MenuItem icon="external-link" highlighted>在浏览器中打开</MenuItem>
    <MenuItem selectable checked detail="1080P">原画</MenuItem>
    <MenuItem icon="bell" disabled>开播提醒</MenuItem>
    <MenuItem icon="trash-2" danger>从历史中移除</MenuItem>
  </Menu>
);
