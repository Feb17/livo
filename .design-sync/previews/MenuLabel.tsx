import { Menu, MenuItem, MenuLabel } from '@livo/design';

export const Groups = () => (
  <Menu>
    <MenuLabel>清晰度</MenuLabel>
    <MenuItem selectable checked>原画</MenuItem>
    <MenuItem selectable>超清</MenuItem>
    <MenuLabel>线路</MenuLabel>
    <MenuItem selectable checked>线路 1</MenuItem>
  </Menu>
);
