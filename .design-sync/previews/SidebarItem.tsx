import { Sidebar, SidebarItem } from '@livo/design';

export const States = () => (
  <div style={{ display: 'flex', width: 220, height: 200 }}>
    <Sidebar trafficLights={false}>
      <SidebarItem icon="compass" active>推荐</SidebarItem>
      <SidebarItem icon="layout-grid">分区</SidebarItem>
      <SidebarItem platform="huya">虎牙</SidebarItem>
      <SidebarItem icon="heart" count="3 在播">关注</SidebarItem>
    </Sidebar>
  </div>
);
