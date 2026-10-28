import { RoomGrid, RoomCard } from '@livo/design';

export const Recommended = () => (
  <div style={{ width: 760 }}>
    <RoomGrid>
      <RoomCard title="王者荣耀巅峰赛 2400 分冲榜" anchor="老王不打野" platform="huya" viewers="12.6万" />
      <RoomCard title="深夜电台：聊聊最近看的书" anchor="慢慢读" platform="bilibili" viewers="2033" />
      <RoomCard title="CS2 职业比赛解说" anchor="战术板" platform="douyu" viewers="9.8万" />
    </RoomGrid>
  </div>
);
