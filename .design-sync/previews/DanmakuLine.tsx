import { Player, DanmakuLine } from '@livo/design';

export const Colors = () => (
  <div style={{ width: 560 }}>
    <Player>
      <DanmakuLine top={12} left={46}>前方高能！！</DanmakuLine>
      <DanmakuLine top={28} left={12}>这操作太稳了</DanmakuLine>
      <DanmakuLine top={44} left={58} color="#FFD23F">主播今天状态好</DanmakuLine>
      <DanmakuLine top={60} left={28} color="#7FE3FF">来了来了</DanmakuLine>
    </Player>
  </div>
);
