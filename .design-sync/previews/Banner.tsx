import { Banner, Button } from '@livo/design';

export const Error = () => (
  <div style={{ width: 560 }}>
    <Banner tone="error" title="无法加载直播间" description="平台接口返回错误，稍后再试或切换线路。" action={<Button>重试</Button>} />
  </div>
);

export const Warning = () => (
  <div style={{ width: 560 }}>
    <Banner tone="warning" title="线路断开，正在重连…" description="已重试 2 次，弹幕暂停接收。" />
  </div>
);

export const Info = () => (
  <div style={{ width: 560 }}>
    <Banner tone="info" title="正在回看 42 秒前的画面" description="点「回到直播」跳到最新。" />
  </div>
);
