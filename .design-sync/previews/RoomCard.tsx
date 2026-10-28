import { RoomCard, RoomGrid } from '@livo/design';

export const Grid = () => (
  <div style={{ width: 760 }}>
    <RoomGrid>
      <RoomCard title="【原神】深渊满星挑战，今晚冲 36 星" anchor="夜猫子小鹿" platform="bilibili" viewers="1.2万" />
      <RoomCard title="周末通宵：艾尔登法环 DLC 全收集" anchor="阿杰打游戏" platform="douyu" viewers="8621" />
      <RoomCard title="点歌请发弹幕，今晚唱到十二点" anchor="唱歌的橙子" platform="huya" viewers="5.3万" />
    </RoomGrid>
  </div>
);

export const Offline = () => (
  <div style={{ width: 240 }}>
    <RoomCard title="深夜电台：聊聊最近看的书" anchor="慢慢读" platform="bilibili" live={false} />
  </div>
);
