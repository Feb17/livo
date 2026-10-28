import { EmptyState, Button } from '@livo/design';

export const NoFollows = () => (
  <EmptyState icon="heart" title="还没有关注的主播" description="进入直播间点「关注」，开播后会排在这里。" action={<Button variant="primary">去看推荐</Button>} />
);
export const NoResults = () => (
  <EmptyState icon="search" title="没有找到「原神深渊」" description="换个关键词，或只搜主播名。" />
);
export const Offline = () => (
  <EmptyState icon="wifi-off" title="网络已断开" description="连上网络后会自动刷新。" action={<Button>重试</Button>} />
);
