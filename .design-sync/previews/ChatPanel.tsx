import { ChatPanel, ChatMessage, SuperChat } from '@livo/design';

export const Live = () => (
  <ChatPanel height={420}>
    <ChatMessage variant="system">小林 进入直播间</ChatMessage>
    <ChatMessage user="路过的猫">这波操作太稳了</ChatMessage>
    <ChatMessage user="阿北">前方高能</ChatMessage>
    <SuperChat user="星河" amount="¥30">主播加油，今天一定能满星！</SuperChat>
    <ChatMessage user="momo">来了来了</ChatMessage>
    <ChatMessage user="雨天">画质好清楚</ChatMessage>
    <ChatMessage variant="system">弹幕连接已恢复</ChatMessage>
  </ChatPanel>
);
export const Reconnecting = () => (
  <ChatPanel height={200} status="reconnecting">
    <ChatMessage variant="system">线路断开，正在重连…</ChatMessage>
  </ChatPanel>
);
