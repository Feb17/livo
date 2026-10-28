import { Window, Sidebar, SidebarSection, SidebarItem, Toolbar, SegmentedControl, SearchField, IconButton, SectionHeader, RoomGrid, RoomCard } from '@livo/design';

export const Home = () => (
  <div style={{ width: 1100 }}>
    <Window
      height={620}
      sidebar={
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
      }
      toolbar={
        <Toolbar title="推荐">
          <SegmentedControl ariaLabel="平台" options={['全部', '哔哩哔哩', '斗鱼', '虎牙']} />
          <SearchField />
          <IconButton icon="rotate-cw" label="刷新" />
        </Toolbar>
      }
    >
      <SectionHeader title="正在直播" note="按人气排序" />
      <RoomGrid>
        <RoomCard title="【原神】深渊满星挑战，今晚冲 36 星" anchor="夜猫子小鹿" platform="bilibili" viewers="1.2万" />
        <RoomCard title="周末通宵：艾尔登法环 DLC 全收集" anchor="阿杰打游戏" platform="douyu" viewers="8621" />
        <RoomCard title="点歌请发弹幕，今晚唱到十二点" anchor="唱歌的橙子" platform="huya" viewers="5.3万" />
        <RoomCard title="王者荣耀巅峰赛 2400 分冲榜" anchor="老王不打野" platform="huya" viewers="12.6万" />
        <RoomCard title="深夜电台：聊聊最近看的书" anchor="慢慢读" platform="bilibili" viewers="2033" />
        <RoomCard title="CS2 职业比赛解说" anchor="战术板" platform="douyu" viewers="9.8万" />
      </RoomGrid>
    </Window>
  </div>
);
