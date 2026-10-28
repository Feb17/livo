import { Sidebar, SidebarSection, SidebarItem } from '@livo/design';

export const Section = () => (
  <div style={{ display: 'flex', width: 220, height: 200 }}>
    <Sidebar trafficLights={false}>
      <SidebarSection title="我的" />
      <SidebarItem icon="heart" count="3 在播">关注</SidebarItem>
      <SidebarItem icon="history">历史</SidebarItem>
    </Sidebar>
  </div>
);
