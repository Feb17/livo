import { AnchorList, AnchorRow } from '@livo/design';

export const Offline = () => (
  <div style={{ width: 640 }}>
    <AnchorList>
      <AnchorRow name="慢慢读" platform="bilibili" title="深夜电台：聊聊最近看的书" live={false} meta="昨天 23:10 开播过" />
      <AnchorRow name="方块建筑师" platform="bilibili" title="Minecraft 生存第 300 天" live={false} meta="3 天前开播过" />
    </AnchorList>
  </div>
);
