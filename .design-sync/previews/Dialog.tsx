import { Dialog } from '@livo/design';

export const Destructive = () => (
  <Dialog title="清空观看历史？" message="将移除全部 128 条记录，此操作无法撤销。" confirmLabel="清空历史" destructive />
);
