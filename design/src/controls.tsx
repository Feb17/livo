import type { ChangeEvent, ReactNode } from 'react';
import { Icon, type IconName } from './core';

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(' ');

export interface ButtonProps {
  /** primary 主操作（一屏只有一个）；secondary 次操作；ghost 取消与工具栏文字按钮；danger 不可撤销的删除 */
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  /** md 28px（默认）；lg 32px，用于搜索栏旁与空状态主操作 */
  size?: 'md' | 'lg';
  /** 前置 16px 图标 */
  icon?: IconName;
  disabled?: boolean;
  /** 按钮文字：2–6 个字、动词开头，例如「关注」「进入直播间」 */
  children?: ReactNode;
  onClick?: () => void;
  type?: 'button' | 'submit';
}

/** 文字按钮。文字动词开头；主按钮用 LIVO 紫填充，一屏只放一个。 */
export function Button({ variant = 'secondary', size = 'md', icon, disabled, children, onClick, type = 'button' }: ButtonProps) {
  return (
    <button
      type={type}
      className={cx('lv-btn', variant !== 'secondary' && `lv-btn-${variant}`, size === 'lg' && 'lv-btn-lg')}
      disabled={disabled}
      onClick={onClick}
    >
      {icon && <Icon name={icon} />}
      {children}
    </button>
  );
}

export interface IconButtonProps {
  /** 20px 图标 */
  icon: IconName;
  /** 动作名，同时作为 aria-label 与 Tooltip 文字 */
  label: string;
  /** 切换类按钮的打开状态，打开时图标变 accent */
  pressed?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

/** 28px 方形图标按钮，用于工具栏与播放器；必须有 label。 */
export function IconButton({ icon, label, pressed, disabled, onClick }: IconButtonProps) {
  return (
    <button type="button" className="lv-icon-btn" aria-label={label} title={label} aria-pressed={pressed} disabled={disabled} onClick={onClick}>
      <Icon name={icon} size={20} />
    </button>
  );
}

export interface SearchFieldProps {
  placeholder?: string;
  value?: string;
  /** 右侧快捷键提示；有输入时自动显示清除按钮 */
  shortcut?: string;
  /** 聚焦态（描边与图标变 accent） */
  focused?: boolean;
  onChange?: (value: string) => void;
  onClear?: () => void;
}

/** 搜索框：同时搜直播间和主播，⌘F 聚焦。 */
export function SearchField({ placeholder = '搜索直播间或主播', value, shortcut = '⌘F', focused, onChange, onClear }: SearchFieldProps) {
  const style = focused ? { borderColor: 'var(--accent)', color: 'var(--accent)' } : undefined;
  return (
    <label className="lv-search" style={style}>
      <Icon name="search" />
      <input
        placeholder={placeholder}
        aria-label={placeholder}
        value={value ?? ''}
        onChange={(e: ChangeEvent<HTMLInputElement>) => onChange?.(e.target.value)}
      />
      {value ? (
        <button type="button" className="lv-icon-btn" style={{ width: 20, height: 20 }} aria-label="清除" onClick={onClear}>
          <Icon name="x" />
        </button>
      ) : (
        shortcut && <span className="lv-kbd">{shortcut}</span>
      )}
    </label>
  );
}

export interface SegmentOption {
  value: string;
  label: string;
}

export interface SegmentedControlProps {
  /** 2–4 个互斥选项 */
  options: (SegmentOption | string)[];
  value?: string;
  onChange?: (value: string) => void;
  ariaLabel?: string;
}

/** 分段控件：2–4 个互斥选项，立即生效（主题、清晰度、平台筛选）。 */
export function SegmentedControl({ options, value, onChange, ariaLabel }: SegmentedControlProps) {
  const opts = options.map((o) => (typeof o === 'string' ? { value: o, label: o } : o));
  const current = value ?? opts[0]?.value;
  return (
    <div className="lv-seg" role="group" aria-label={ariaLabel}>
      {opts.map((o) => (
        <button key={o.value} type="button" aria-pressed={o.value === current} onClick={() => onChange?.(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

export interface ToggleProps {
  checked?: boolean;
  /** 无障碍标签（设置项名称） */
  label: string;
  onChange?: (checked: boolean) => void;
}

/** 开关：切换一个立即生效的设置；放在 SettingRow 的右侧。 */
export function Toggle({ checked = false, label, onChange }: ToggleProps) {
  return <button type="button" className="lv-toggle" role="switch" aria-checked={checked} aria-label={label} onClick={() => onChange?.(!checked)} />;
}

export interface SliderProps {
  value: number;
  min?: number;
  max?: number;
  /** 标签，显示在左上 */
  label?: string;
  /** 当前值的显示文字（带单位），显示在右上，例如「8 秒」「85%」 */
  valueLabel?: string;
  /** default 设置页；media 播放器上（白色半透明轨道） */
  variant?: 'default' | 'media';
  /** 播放器进度条的缓冲段（0–100） */
  buffered?: number;
  onChange?: (value: number) => void;
}

/** 滑块：调连续值（弹幕速度、不透明度、音量、时移进度）。 */
export function Slider({ value, min = 0, max = 100, label, valueLabel, variant = 'default', buffered, onChange }: SliderProps) {
  const pct = Math.max(0, Math.min(100, ((value - min) / (max - min)) * 100));
  const track = (
    <div className={cx('lv-slider', variant === 'media' && 'lv-slider-media')}>
      <div className="lv-slider-track">
        {variant === 'media' && buffered !== undefined && <div className="lv-slider-buffer" style={{ width: `${buffered}%` }} />}
        <div className="lv-slider-fill" style={{ width: `${pct}%` }} />
        <div className="lv-slider-thumb" style={{ left: `${pct}%` }} />
      </div>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        aria-label={label}
        onChange={(e) => onChange?.(Number(e.target.value))}
        style={{ position: 'absolute', inset: 0, opacity: 0, width: '100%', margin: 0, cursor: 'pointer' }}
      />
    </div>
  );
  if (!label) return track;
  return (
    <div className="lv-field">
      <div className="lv-field-head">
        <span>{label}</span>
        {valueLabel && <span>{valueLabel}</span>}
      </div>
      {track}
    </div>
  );
}

export interface PopupButtonProps {
  /** 当前选中值 */
  value: string;
  /** 播放器上用 media 外观 */
  variant?: 'default' | 'media';
  onClick?: () => void;
}

/** 弹出按钮：显示当前值 + 下拉箭头，点击后弹出 Menu（清晰度、线路、启动页）。 */
export function PopupButton({ value, variant = 'default', onClick }: PopupButtonProps) {
  return (
    <button type="button" className={variant === 'media' ? 'lv-popup-media' : 'lv-popup'} onClick={onClick}>
      {value}
      <Icon name="chevron-down" />
    </button>
  );
}

export interface KeybindingFieldProps {
  /** 当前快捷键，按 macOS 顺序：⌃ ⌥ ⇧ ⌘ 再键名，例如 ['⌘', 'F'] */
  keys?: string[];
  /** 录入中：等待按下新的快捷键 */
  recording?: boolean;
  /** 冲突说明，例如「与「搜索」冲突」；有值时描边变 danger */
  conflict?: string;
  onClick?: () => void;
}

/** 快捷键录入框（设置 → 快捷键）。 */
export function KeybindingField({ keys = [], recording, conflict, onClick }: KeybindingFieldProps) {
  const field = (
    <button type="button" className={cx('lv-keyfield', recording && 'is-recording', conflict && 'is-conflict')} onClick={onClick}>
      {recording ? '按下新的快捷键…' : keys.map((k) => <span key={k} className="lv-keycap">{k}</span>)}
    </button>
  );
  if (!conflict) return field;
  return (
    <span style={{ display: 'inline-flex', flexDirection: 'column', alignItems: 'flex-end', gap: 4 }}>
      {field}
      <span style={{ fontSize: 11, color: 'var(--danger)' }}>{conflict}</span>
    </span>
  );
}
