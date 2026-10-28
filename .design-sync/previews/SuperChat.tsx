import { ChatPanel, ChatMessage, SuperChat } from '@livo/design';

export const InChat = () => (
  <ChatPanel height={260}>
    <ChatMessage user="阿北">前方高能</ChatMessage>
    <SuperChat user="星河" amount="¥30">主播加油，今天一定能满星！</SuperChat>
    <ChatMessage user="momo">来了来了</ChatMessage>
  </ChatPanel>
);
