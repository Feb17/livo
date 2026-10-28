import { Toolbar, SegmentedControl, SearchField, IconButton } from '@livo/design';

export const Home = () => (
  <div style={{ width: 760, background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 10 }}>
    <Toolbar title="推荐">
      <SegmentedControl ariaLabel="平台" options={['全部', '哔哩哔哩', '斗鱼', '虎牙']} />
      <SearchField />
      <IconButton icon="rotate-cw" label="刷新" />
    </Toolbar>
  </div>
);
export const Room = () => (
  <div style={{ width: 760, background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 10 }}>
    <Toolbar title="夜猫子小鹿" subtitle="哔哩哔哩 · 直播间">
      <SearchField />
    </Toolbar>
  </div>
);
