import { SegmentedControl } from '@livo/design';

export const PlatformFilter = () => (
  <SegmentedControl ariaLabel="平台" options={['全部', '哔哩哔哩', '斗鱼', '虎牙']} value="全部" />
);
export const Appearance = () => (
  <SegmentedControl ariaLabel="外观" options={['跟随系统', '深色', '浅色']} value="深色" />
);
