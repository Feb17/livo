import { Sidebar, SidebarSection, SidebarItem } from '@livo/design';

export const Navigation = () => (
  <div style={{ height: 380, display: 'flex', width: 220 }}>
    <Sidebar>
      <SidebarSection title="发现" />
      <SidebarItem icon="compass" active>推荐</SidebarItem>
      <SidebarItem icon="layout-grid">分区</SidebarItem>
      <SidebarSection title="平台" />
      <SidebarItem platform="bilibili">哔哩哔哩</SidebarItem>
      <SidebarItem platform="douyu">斗鱼</SidebarItem>
      <SidebarItem platform="huya">虎牙</SidebarItem>
      <SidebarSection title="我的" />
      <SidebarItem icon="heart" count="3 在播">关注</SidebarItem>
      <SidebarItem icon="history">历史</SidebarItem>
    </Sidebar>
  </div>
);
