import { Player, PlayerControls } from '@livo/design';

export const Live = () => (
  <div style={{ width: 720 }}>
    <Player><PlayerControls /></Player>
  </div>
);
export const TimeShift = () => (
  <div style={{ width: 720 }}>
    <Player><PlayerControls live={false} offset="-00:42" progress={86} danmaku={false} /></Player>
  </div>
);
