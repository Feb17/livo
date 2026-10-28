import type { ReactNode } from 'react';
import { Icon, Logo, PLATFORM_NAMES, type IconName, type Platform } from './core';
import { Button } from './controls';

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(' ');

export interface BadgeProps {
  /** live：青底 LIVE；quality：清晰度；neutral：中性信息；media：压在封面/画面上的半透明底 */
  tone?: 'live' | 'quality' | 'neutral' | 'media';
  /** 12px 前置图标（如 media 徽标里的 users） */
  icon?: IconName;
  children?: ReactNode;
}

/** 徽标：LIVE、清晰度、未开播、人气数。不可点击，1–6 个字。 */
export function Badge({ tone = 'neutral', icon, children }: BadgeProps) {
  return (
    <span className={cx('lv-badge', `lv-badge-${tone}`)}>
      {icon && <Icon name={icon} />}
      {children}
    </span>
  );
}

export interface PlatformTagProps {
  platform: Platform;
}

/** 平台标签：平台色圆点 + 平台名（哔哩哔哩 / 斗鱼 / 虎牙）。平台色只用在这里和侧栏圆点。 */
export function PlatformTag({ platform }: PlatformTagProps) {
  return <span className={cx('lv-platform', platform)}>{PLATFORM_NAMES[platform]}</span>;
}

export interface PlatformDotProps {
  platform: Platform;
}

/** 8px 平台色圆点，用于侧栏平台项与筛选图例，总是配平台名文字。 */
export function PlatformDot({ platform }: PlatformDotProps) {
  return <span className={cx('lv-pf-dot', platform)} aria-hidden="true" />;
}

export interface LiveStatusProps {
  /** true：青色「直播中」；false：灰色「未开播」 */
  live?: boolean;
  /** 覆盖默认文字，例如「实时」「重连中」 */
  label?: string;
}

/** 直播状态：6px 圆点 + 文字。颜色只是辅助，永远配文字。 */
export function LiveStatus({ live = true, label }: LiveStatusProps) {
  return (
    <span className={cx('lv-status', !live && 'lv-status-off')}>
      <span className="lv-dot" />
      {label ?? (live ? '直播中' : '未开播')}
    </span>
  );
}

export interface AvatarProps {
  /** 主播名；无图时显示首字 */
  name?: string;
  src?: string;
  /** sm 24（卡片）、md 32（弹幕栏）、lg 40（关注列表）、xl 56（直播间信息栏） */
  size?: 'sm' | 'md' | 'lg' | 'xl';
  /** 在播时加青色外环 */
  live?: boolean;
}

/** 圆形头像：图片 → 名字首字 → user 图标三级缺省；在播加青色环。 */
export function Avatar({ name, src, size = 'md', live }: AvatarProps) {
  return (
    <span className={cx('lv-av', size !== 'md' && `lv-av-${size}`, live && 'lv-av-live')}>
      {src ? <img src={src} alt={name ?? ''} /> : name ? name.slice(0, 1) : <Icon name="user" />}
    </span>
  );
}

export interface MenuProps {
  children?: ReactNode;
  ariaLabel?: string;
}

/** 菜单面板（弹出菜单与右键菜单），里面放 MenuItem / MenuSeparator / MenuLabel。 */
export function Menu({ children, ariaLabel }: MenuProps) {
  return (
    <div className="lv-menu" role="menu" aria-label={ariaLabel}>
      {children}
    </div>
  );
}

export interface MenuItemProps {
  icon?: IconName;
  /** 单选菜单项：显示勾选 */
  checked?: boolean;
  /** 单选菜单（清晰度、线路）设 true，项目左侧留出勾选位 */
  selectable?: boolean;
  /** 右侧快捷键，例如 ⌘C */
  shortcut?: string;
  /** 右侧补充信息，例如 1080P */
  detail?: string;
  /** 高亮（键盘或鼠标停留）状态 */
  highlighted?: boolean;
  /** 破坏性操作，放在菜单最后 */
  danger?: boolean;
  disabled?: boolean;
  children?: ReactNode;
  onSelect?: () => void;
}

/** 菜单项。高亮用 LIVO 紫填充（macOS 菜单惯例），破坏性项高亮为红。 */
export function MenuItem({ icon, checked, selectable, shortcut, detail, highlighted, danger, disabled, children, onSelect }: MenuItemProps) {
  return (
    <div
      className={cx('lv-menu-item', highlighted && 'is-hover', danger && 'danger')}
      role={selectable ? 'menuitemradio' : 'menuitem'}
      aria-checked={selectable ? !!checked : undefined}
      aria-disabled={disabled || undefined}
      onClick={disabled ? undefined : onSelect}
    >
      {selectable ? <Icon name="check" className="lv-check" /> : icon && <Icon name={icon} />}
      {children}
      {detail && <span className="lv-sub">{detail}</span>}
      {shortcut && <span className="lv-key">{shortcut}</span>}
    </div>
  );
}

/** 菜单分隔线：隔开不同组的命令，破坏性操作前必须有。 */
export function MenuSeparator() {
  return <div className="lv-menu-sep" role="separator" />;
}

export interface MenuLabelProps {
  children?: ReactNode;
}

/** 菜单分组标题（如「清晰度」「线路」）。 */
export function MenuLabel({ children }: MenuLabelProps) {
  return <div className="lv-menu-label">{children}</div>;
}

export interface DialogProps {
  /** 问句标题，说清对象：「清空观看历史？」 */
  title: string;
  /** 一句说明范围与能否撤销 */
  message?: string;
  /** 主按钮文字，动词，与标题对应 */
  confirmLabel: string;
  cancelLabel?: string;
  /** 破坏性操作：主按钮为红色 */
  destructive?: boolean;
  onConfirm?: () => void;
  onCancel?: () => void;
}

/** macOS 警告框：只用于不可撤销操作的确认。顶部 App 图标，按钮纵向铺满。 */
export function Dialog({ title, message, confirmLabel, cancelLabel = '取消', destructive, onConfirm, onCancel }: DialogProps) {
  return (
    <div className="lv-alert" role="alertdialog" aria-label={title}>
      <div style={{ display: 'flex', justifyContent: 'center', marginBottom: 'var(--space-3)' }}>
        <Logo variant="app-icon" height={56} />
      </div>
      <h4>{title}</h4>
      {message && <p>{message}</p>}
      <div className="lv-stack">
        <Button variant={destructive ? 'danger' : 'primary'} onClick={onConfirm}>{confirmLabel}</Button>
        <Button onClick={onCancel}>{cancelLabel}</Button>
      </div>
    </div>
  );
}

export interface BannerProps {
  /** error 错误；warning 警告（断流重连中）；info 提示（时移中） */
  tone?: 'info' | 'warning' | 'error';
  /** 一句话说发生了什么 */
  title: string;
  /** 原因或下一步 */
  description?: string;
  /** 一个可选的动作按钮，例如 <Button>重试</Button> */
  action?: ReactNode;
}

const BANNER_ICON = { info: 'info', warning: 'triangle-alert', error: 'circle-alert' } as const;

/** 窗口内提示条：放在内容区顶部，不遮挡画面；问题解决前一直显示。 */
export function Banner({ tone = 'info', title, description, action }: BannerProps) {
  return (
    <div className={cx('lv-banner', `lv-banner-${tone}`)} role={tone === 'error' ? 'alert' : 'status'}>
      <Icon name={BANNER_ICON[tone]} />
      <div className="lv-banner-body">
        <b>{title}</b>
        {description && <span>{description}</span>}
      </div>
      {action}
    </div>
  );
}

export interface ToastProps {
  icon?: IconName;
  children?: ReactNode;
}

/** Toast：短操作的结果（已复制、已关注），窗口底部居中，2 秒后淡出，不放按钮。 */
export function Toast({ icon = 'check', children }: ToastProps) {
  return (
    <span className="lv-toast" role="status">
      <Icon name={icon} />
      {children}
    </span>
  );
}

export interface TooltipProps {
  /** 动作名（切换类写下一步动作：「关闭弹幕」） */
  label: string;
  /** 快捷键，例如 ⌥⌘P */
  shortcut?: string;
}

/** 悬停提示气泡：图标按钮的文字说明与快捷键，悬停 0.5 秒后出现在按钮上方。 */
export function Tooltip({ label, shortcut }: TooltipProps) {
  return (
    <span className="lv-tooltip" role="tooltip">
      {label}
      {shortcut && <span className="lv-kbd">{shortcut}</span>}
    </span>
  );
}

export interface EmptyStateProps {
  icon: IconName;
  /** 一句标题，说现状；搜索无结果时回显关键词 */
  title: string;
  /** 一句说明，说下一步 */
  description?: string;
  /** 可选按钮 */
  action?: ReactNode;
}

/** 空状态：列表为空、搜索无结果、断网时的占位。不放插画与 emoji。 */
export function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="lv-empty">
      <div className="lv-empty-icon">
        <Icon name={icon} />
      </div>
      <h4>{title}</h4>
      {description && <p>{description}</p>}
      {action}
    </div>
  );
}

export interface SkeletonProps {
  /** card：16:9 封面 + 两行标题；row：头像 + 两行文字；line：单行 */
  variant?: 'card' | 'row' | 'line';
  /** line 的宽度，例如 '60%' */
  width?: string;
}

/** 骨架占位：加载超过 300ms 的卡片与列表，形状与真实内容一致；不用转圈代替。 */
export function Skeleton({ variant = 'card', width = '100%' }: SkeletonProps) {
  if (variant === 'line') return <div className="lv-skel" style={{ height: 12, width }} />;
  if (variant === 'row')
    return (
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <div className="lv-skel" style={{ width: 40, height: 40, borderRadius: 999, flex: 'none' }} />
        <div className="lv-stack" style={{ flex: 1, gap: 8 }}>
          <div className="lv-skel" style={{ height: 12, width: '40%' }} />
          <div className="lv-skel" style={{ height: 12, width: '70%' }} />
        </div>
      </div>
    );
  return (
    <div className="lv-stack" style={{ gap: 8 }}>
      <div className="lv-skel" style={{ aspectRatio: '16 / 9', borderRadius: 'var(--radius-md)' }} />
      <div className="lv-skel" style={{ height: 12, width: '90%' }} />
      <div className="lv-skel" style={{ height: 12, width: '60%' }} />
    </div>
  );
}
