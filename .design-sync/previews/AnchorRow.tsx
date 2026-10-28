import { AnchorList, AnchorRow } from '@livo/design';

export const Follows = () => (
  <div style={{ width: 640 }}>
    <AnchorList>
      <AnchorRow name="夜猫子小鹿" platform="bilibili" title="【原神】深渊满星挑战，今晚冲 36 星" meta="1.2万 人气" />
      <AnchorRow name="阿杰打游戏" platform="douyu" title="周末通宵：艾尔登法环 DLC 全收集" meta="8621 人气" />
      <AnchorRow name="唱歌的橙子" platform="huya" title="点歌请发弹幕" live={false} meta="3 小时前开播过" />
    </AnchorList>
  </div>
);
