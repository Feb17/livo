import type { ReactNode } from 'react';
import { Icon, type Platform } from './core';
import { Avatar, Badge, LiveStatus, PlatformTag } from './feedback';
import { IconButton } from './controls';

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(' ');

export interface CoverProps {
  /** 封面地址；不传时显示占位色块 */
  src?: string;
  alt?: string;
  /** loaded 已加载；loading 加载中（色块）；error 加载失败（image-off）；offline 未开播（去色 + 45% 不透明） */
  state?: 'loaded' | 'loading' | 'error' | 'offline';
  /** 左上 LIVE 徽标 */
  live?: boolean;
  /** 左下人气数，例如「1.2万」 */
  viewers?: string;
}

/** 16:9 封面：居中裁切铺满、圆角、1px hairline 内描边；只叠 LIVE 与人气两枚徽标。 */
export function Cover({ src, alt = '', state = 'loaded', live, viewers }: CoverProps) {
  return (
    <div className={cx('lv-cover', state === 'offline' && 'lv-cover-offline')}>
      {state === 'loading' && <div className="lv-skel" style={{ position: 'absolute', inset: 0, borderRadius: 0 }} />}
      {state === 'error' && (
        <div className="lv-cover-fallback">
          <Icon name="image-off" size={20} />
        </div>
      )}
      {(state === 'loaded' || state === 'offline') && (src ? <img src={src} alt={alt} /> : <div className="lv-art" />)}
      {state === 'offline' ? (
        <span className="lv-badge lv-badge-media lv-tl">未开播</span>
      ) : (
        <>
          {live && <span className="lv-badge lv-badge-live lv-tl">LIVE</span>}
          {viewers && (
            <span className="lv-badge lv-badge-media lv-bl">
              <Icon name="users" />
              {viewers}
            </span>
          )}
        </>
      )}
    </div>
  );
}

export interface RoomCardProps {
  /** 直播间标题，最多两行 */
  title: string;
  /** 主播名 */
  anchor: string;
  platform: Platform;
  /** 人气数，按中文习惯缩写：「1.2万」 */
  viewers?: string;
  live?: boolean;
  coverSrc?: string;
  avatarSrc?: string;
  onClick?: () => void;
}

/** 直播间卡片：封面 + 标题 + 主播与平台。成网格排列（lv-grid，最小 220px）。 */
export function RoomCard({ title, anchor, platform, viewers, live = true, coverSrc, avatarSrc, onClick }: RoomCardProps) {
  return (
    <a className="lv-card" href="#" onClick={(e) => { e.preventDefault(); onClick?.(); }}>
      <Cover src={coverSrc} state={live ? 'loaded' : 'offline'} live={live} viewers={live ? viewers : undefined} />
      <div className="lv-card-title">{title}</div>
      <div className="lv-card-meta">
        <Avatar name={anchor} src={avatarSrc} size="sm" />
        <span className="lv-card-name">{anchor}</span>
        <PlatformTag platform={platform} />
      </div>
    </a>
  );
}

export interface RoomGridProps {
  children?: ReactNode;
}

/** 直播间卡片网格：按 card-min 220px 自动分列，间距 space-4。 */
export function RoomGrid({ children }: RoomGridProps) {
  return <div className="lv-grid">{children}</div>;
}

export interface AnchorRowProps {
  name: string;
  platform: Platform;
  /** 当前直播标题（未开播时为上次标题） */
  title: string;
  live?: boolean;
  /** 右侧数字：在播时人气（「1.2万 人气」），未开播时上次开播时间（「3 小时前开播过」） */
  meta?: string;
  avatarSrc?: string;
  onClick?: () => void;
}

/** 主播行：关注列表与搜索结果。在播的头像有青色环，整行可点进入直播间。放在 AnchorList 里。 */
export function AnchorRow({ name, platform, title, live = true, meta, avatarSrc, onClick }: AnchorRowProps) {
  return (
    <div className="lv-anchor" role="button" tabIndex={0} onClick={onClick}>
      <Avatar name={name} src={avatarSrc} size="lg" live={live} />
      <div className="lv-anchor-main">
        <div className="lv-anchor-name">
          {name}
          <PlatformTag platform={platform} />
        </div>
        <div className="lv-anchor-title">{title}</div>
      </div>
      <div className="lv-anchor-meta">
        <LiveStatus live={live} />
        {meta && <div>{meta}</div>}
      </div>
      <Icon name="chevron-right" />
    </div>
  );
}

export interface AnchorListProps {
  children?: ReactNode;
}

/** AnchorRow 的容器：带描边的分组列表。 */
export function AnchorList({ children }: AnchorListProps) {
  return <div className="lv-list">{children}</div>;
}

export interface ChatPanelProps {
  /** 连接状态：live「实时」；reconnecting「重连中」 */
  status?: 'live' | 'reconnecting';
  /** 消息流：ChatMessage / SuperChat */
  children?: ReactNode;
  /** 面板高度（px），默认撑满父容器 */
  height?: number;
}

/** 直播间右侧弹幕栏（宽 320）：头部状态与两个图标按钮，下方消息流从底部向上排。 */
export function ChatPanel({ status = 'live', children, height }: ChatPanelProps) {
  return (
    <aside className="lv-chat" style={height ? { height } : { alignSelf: 'stretch' }}>
      <div className="lv-chat-head">
        <b>弹幕</b>
        <LiveStatus live={status === 'live'} label={status === 'live' ? '实时' : '重连中'} />
        <span style={{ flex: 1 }} />
        <IconButton icon="pause" label="暂停滚动" />
        <IconButton icon="sliders-horizontal" label="弹幕设置" />
      </div>
      <div className="lv-chat-body">{children}</div>
    </aside>
  );
}

export interface ChatMessageProps {
  /** 用户名；system 消息不传 */
  user?: string;
  /** message 普通弹幕；system 系统消息（进入直播间、连接恢复） */
  variant?: 'message' | 'system';
  children?: ReactNode;
}

/** 弹幕栏里的一条消息：用户名 text-muted 600 + 内容；系统消息 12px text-subtle。 */
export function ChatMessage({ user, variant = 'message', children }: ChatMessageProps) {
  if (variant === 'system') return <div className="lv-msg lv-msg-system">{children}</div>;
  return (
    <div className="lv-msg">
      {user && <span className="lv-msg-name">{user}</span>}
      {children}
    </div>
  );
}

export interface SuperChatProps {
  user: string;
  /** 金额，例如「¥30」 */
  amount: string;
  children?: ReactNode;
}

/** 醒目留言（SC）：LIVO 紫头部（头像、名字、金额）+ 正文，按到达顺序插在弹幕流里。 */
export function SuperChat({ user, amount, children }: SuperChatProps) {
  return (
    <div className="lv-sc">
      <div className="lv-sc-head">
        <Avatar name={user} size="sm" />
        {user}
        <span className="lv-sc-price">{amount}</span>
      </div>
      <div className="lv-sc-body">{children}</div>
    </div>
  );
}

export interface RoomInfoProps {
  title: string;
  anchor: string;
  platform: Platform;
  viewers?: string;
  /** 开播时长，例如「开播 2 小时 14 分」 */
  duration?: string;
  avatarSrc?: string;
  live?: boolean;
  /** 当前清晰度 */
  quality?: string;
  /** 右侧操作区，通常是 <Button variant="primary" icon="plus">关注</Button> */
  actions?: ReactNode;
}

/** 直播间信息栏：播放器下方，56px 头像 + 标题 + 主播、平台、人气、时长 + 右侧操作。 */
export function RoomInfo({ title, anchor, platform, viewers, duration, avatarSrc, live = true, quality, actions }: RoomInfoProps) {
  return (
    <div className="lv-room-info">
      <Avatar name={anchor} src={avatarSrc} size="xl" live={live} />
      <div className="lv-room-info-main">
        <div className="lv-room-info-title">{title}</div>
        <div className="lv-room-info-meta">
          <span style={{ color: 'var(--text)', fontWeight: 600 }}>{anchor}</span>
          <PlatformTag platform={platform} />
          {viewers && (
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
              <Icon name="users" />
              {viewers}
            </span>
          )}
          {duration && <span>{duration}</span>}
        </div>
      </div>
      {quality && <Badge tone="quality">{quality}</Badge>}
      {actions}
    </div>
  );
}
