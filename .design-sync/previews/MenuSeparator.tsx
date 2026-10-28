import { Menu, MenuItem, MenuSeparator } from '@livo/design';

export const BeforeDestructive = () => (
  <Menu>
    <MenuItem icon="play">进入直播间</MenuItem>
    <MenuItem icon="copy" shortcut="⌘C">复制链接</MenuItem>
    <MenuSeparator />
    <MenuItem icon="trash-2" danger>取消关注</MenuItem>
  </Menu>
);
