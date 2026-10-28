import type { ReactNode } from 'react';
import { Icon, type IconName, type Platform } from './core';
import { IconButton } from './controls';
import { PlatformDot } from './feedback';

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(' ');

/** macOS 红绿灯（关闭 / 最小化 / 全屏），放在侧栏或标题栏左上角。 */
export function TrafficLights({ fullscreenDisabled }: { fullscreenDisabled?: boolean }) {
  return (
    <div className="lv-traffic" aria-hidden="true">
      <i />
      <i />
      <i style={fullscreenDisabled ? { opacity: 0.35 } : undefined} />
    </div>
  );
}

export interface SidebarProps {
  /** SidebarSection 与 SidebarItem */
  children?: ReactNode;
  /** 在顶部显示红绿灯（窗口里的侧栏都要开） */
  trafficLights?: boolean;
}

/** 侧栏（macOS 源列表）：宽 220，sidebar 底色，分组「发现 / 平台 / 我的」。 */
export function Sidebar({ children, trafficLights = true }: SidebarProps) {
  return (
    <aside className="lv-wside" aria-label="侧栏">
      {trafficLights && <TrafficLights />}
      {children}
    </aside>
  );
}

export interface SidebarSectionProps {
  title: string;
}

/** 侧栏分组标题：11px 600 text-subtle。 */
export function SidebarSection({ title }: SidebarSectionProps) {
  return <div className="lv-nav-label">{title}</div>;
}

export interface SidebarItemProps {
  /** 16px 图标；平台项改用 platform */
  icon?: IconName;
  /** 平台项：显示平台色圆点 */
  platform?: Platform;
  /** 当前页 */
  active?: boolean;
  /** 右侧计数，例如「3 在播」 */
  count?: string;
  children?: ReactNode;
  onClick?: () => void;
}

/** 侧栏导航项：选中为 accent-soft 底 + accent 字。设置不放侧栏（走 ⌘,）。 */
export function SidebarItem({ icon, platform, active, count, children, onClick }: SidebarItemProps) {
  return (
    <a className="lv-nav-item" href="#" aria-current={active ? 'page' : undefined} onClick={(e) => { e.preventDefault(); onClick?.(); }}>
      {platform ? <span style={{ margin: '0 4px', display: 'inline-flex' }}><PlatformDot platform={platform} /></span> : icon && <Icon name={icon} />}
      {children}
      {count && <span className="lv-count">{count}</span>}
    </a>
  );
}

export interface ToolbarProps {
  /** 页面标题 */
  title: string;
  /** 标题旁的补充说明 */
  subtitle?: string;
  /** 右侧区域：筛选、搜索框、刷新等 */
  children?: ReactNode;
  /** 显示后退 / 前进 */
  navigation?: boolean;
  canGoForward?: boolean;
}

/** 统一工具栏（高 52）：后退 / 前进、页面标题，右侧放当前页面的高频操作。 */
export function Toolbar({ title, subtitle, children, navigation = true, canGoForward = false }: ToolbarProps) {
  return (
    <div className="lv-toolbar">
      {navigation && (
        <>
          <IconButton icon="chevron-left" label="后退" />
          <IconButton icon="chevron-right" label="前进" disabled={!canGoForward} />
        </>
      )}
      <span className="lv-toolbar-title">{title}</span>
      {subtitle && <span className="lv-toolbar-sub">{subtitle}</span>}
      <span className="lv-spacer" />
      {children}
    </div>
  );
}

export interface WindowProps {
  /** 侧栏（通常是 <Sidebar>）；不传则为无侧栏窗口 */
  sidebar?: ReactNode;
  /** 工具栏（通常是 <Toolbar>） */
  toolbar?: ReactNode;
  /** 内容区 */
  children?: ReactNode;
  /** 窗口高度（px），默认 780 */
  height?: number;
  /** 内容区是否带 24px 页边距（直播间页面关掉） */
  padded?: boolean;
}

/** macOS 主窗口框架：侧栏 + 统一工具栏 + 内容区。所有页面都放在它里面。 */
export function Window({ sidebar, toolbar, children, height = 780, padded = true }: WindowProps) {
  return (
    <div className={cx('lv-window', !sidebar && 'no-sidebar')} style={{ height }}>
      {sidebar}
      <div className="lv-wmain">
        {toolbar}
        {padded ? <div className="lv-wcontent">{children}</div> : <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>{children}</div>}
      </div>
    </div>
  );
}

export interface SectionHeaderProps {
  title: string;
  /** 右侧说明或数量 */
  note?: string;
}

/** 内容区分节标题：17/22 600 + 12px 说明。 */
export function SectionHeader({ title, note }: SectionHeaderProps) {
  return (
    <div className="lv-section-head">
      <h3>{title}</h3>
      {note && <span>{note}</span>}
    </div>
  );
}

export interface SettingsTab {
  icon: IconName;
  label: string;
}

export interface SettingsWindowProps {
  /** 工具栏标签，例如 通用 / 播放 / 弹幕 / 快捷键 */
  tabs: SettingsTab[];
  /** 当前标签（也是窗口标题） */
  active: string;
  /** SettingsGroup 列表 */
  children?: ReactNode;
}

/** 设置窗口（⌘, 打开）：宽 600、不可调整大小，工具栏标签切换分类，修改立即生效。 */
export function SettingsWindow({ tabs, active, children }: SettingsWindowProps) {
  return (
    <div className="lv-settings">
      <div className="lv-settings-head">
        <TrafficLights fullscreenDisabled />
        <div className="lv-settings-title">{active}</div>
        <div className="lv-settings-tabs" role="tablist">
          {tabs.map((t) => (
            <button key={t.label} type="button" className="lv-settings-tab" role="tab" aria-selected={t.label === active}>
              <Icon name={t.icon} size={20} />
              <span>{t.label}</span>
            </button>
          ))}
        </div>
      </div>
      <div className="lv-settings-body">{children}</div>
    </div>
  );
}

export interface SettingsGroupProps {
  title?: string;
  /** SettingRow 列表 */
  children?: ReactNode;
}

/** 设置分组：11px 分组标题 + 带描边的卡片。 */
export function SettingsGroup({ title, children }: SettingsGroupProps) {
  return (
    <div>
      {title && <div className="lv-group-title">{title}</div>}
      <div className="lv-group">{children}</div>
    </div>
  );
}

export interface SettingRowProps {
  title: string;
  /** 一行说明，text-muted 12px */
  description?: string;
  /** 右侧控件：Toggle、SegmentedControl、PopupButton、Button、KeybindingField */
  control?: ReactNode;
}

/** 设置行：左侧名称与说明，右侧控件。 */
export function SettingRow({ title, description, control }: SettingRowProps) {
  return (
    <div className="lv-setting-row">
      <div>
        <div>{title}</div>
        {description && <div className="lv-setting-sub">{description}</div>}
      </div>
      {control}
    </div>
  );
}
