import { ChatPanel, ChatMessage } from '@livo/design';

export const Kinds = () => (
  <ChatPanel height={220}>
    <ChatMessage variant="system">小林 进入直播间</ChatMessage>
    <ChatMessage user="路过的猫">这波操作太稳了</ChatMessage>
    <ChatMessage user="阿北">前方高能</ChatMessage>
    <ChatMessage variant="system">弹幕连接已恢复</ChatMessage>
  </ChatPanel>
);
