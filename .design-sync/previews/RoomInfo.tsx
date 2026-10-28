import { RoomInfo, Button, IconButton } from '@livo/design';

export const Default = () => (
  <div style={{ width: 820, background: 'var(--bg)' }}>
    <RoomInfo
      title="【原神】深渊满星挑战，今晚冲 36 星"
      anchor="夜猫子小鹿"
      platform="bilibili"
      viewers="1.2万"
      duration="开播 2 小时 14 分"
      quality="原画"
      actions={<><IconButton icon="copy" label="复制链接" /><IconButton icon="ellipsis" label="更多" /><Button variant="primary" icon="plus">关注</Button></>}
    />
  </div>
);
