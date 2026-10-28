import { Player, DanmakuLine, PlayerControls } from '@livo/design';

export const Live = () => (
  <div style={{ width: 640 }}>
    <Player>
      <DanmakuLine top={14} left={50}>前方高能</DanmakuLine>
      <DanmakuLine top={28} left={12}>这操作太稳了</DanmakuLine>
      <PlayerControls />
    </Player>
  </div>
);
export const Fullscreen = () => (
  <div style={{ width: 640 }}>
    <Player title="【原神】深渊满星挑战，今晚冲 36 星">
      <PlayerControls />
    </Player>
  </div>
);
