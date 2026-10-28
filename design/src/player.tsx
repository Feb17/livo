import type { CSSProperties, ReactNode } from 'react';
import { Icon } from './core';
import { IconButton, PopupButton, Slider } from './controls';

export interface PlayerProps {
  /** 视频或截图地址；不传时显示深色示意画面 */
  src?: string;
  /** 全屏时顶部显示的直播间标题 */
  title?: string;
  /** 画面上的 DanmakuLine 与底部 PlayerControls */
  children?: ReactNode;
  /** fill：撑满父容器（直播间页面，画面按 contain 居中）；默认 16:9 */
  fill?: boolean;
}

/** 播放器画面：永远深色、不跟随主题，内含弹幕层与悬浮控制条。 */
export function Player({ src, title, children, fill }: PlayerProps) {
  const style: CSSProperties = fill ? { flex: 1, minHeight: 0, aspectRatio: 'auto', borderRadius: 0 } : {};
  return (
    <div className="lv-player" style={style}>
      {src ? <img src={src} alt="" /> : <div className="lv-player-art" />}
      {title && <div className="lv-player-title">{title}</div>}
      {children}
    </div>
  );
}

export interface DanmakuLineProps {
  children?: ReactNode;
  /** 距画面顶部（%），弹幕只占上方 3/4 */
  top: number;
  /** 距画面左侧（%） */
  left: number;
  /** 用户自定义颜色，原样显示；默认白色 */
  color?: string;
}

/** 一条滚动弹幕：20/26 600、3px 描边，在任何画面上可读。 */
export function DanmakuLine({ children, top, left, color }: DanmakuLineProps) {
  return (
    <span className="lv-danmaku" style={{ top: `${top}%`, left: `${left}%`, color }}>
      {children}
    </span>
  );
}

export interface PlayerControlsProps {
  playing?: boolean;
  /** true：在直播边缘，显示 LIVE；false：时移中，显示偏移与「回到直播」 */
  live?: boolean;
  /** 时移偏移，例如「-00:42」 */
  offset?: string;
  /** 时移进度（0–100），live 时为 100 */
  progress?: number;
  volume?: number;
  quality?: string;
  line?: string;
  /** 弹幕是否打开 */
  danmaku?: boolean;
}

/** 悬浮在画面底部的播放控制条：进度 + 播放/直播状态/音量 + 清晰度、线路、弹幕、画中画、全屏。 */
export function PlayerControls({ playing = true, live = true, offset = '-00:42', progress, volume = 70, quality = '原画', line = '线路 1', danmaku = true }: PlayerControlsProps) {
  const p = live ? 100 : progress ?? 86;
  return (
    <div className="lv-pc">
      <div className="lv-pc-row">
        <Slider value={p} variant="media" buffered={100} />
      </div>
      <div className="lv-pc-row">
        <IconButton icon={playing ? 'pause' : 'play'} label={playing ? '暂停' : '播放'} />
        {live ? (
          <span className="lv-badge lv-badge-live" style={{ margin: '0 6px' }}>LIVE</span>
        ) : (
          <>
            <span className="lv-timecode" style={{ margin: '0 6px' }}>{offset}</span>
            <button type="button" className="lv-golive">
              <Icon name="radio" /> 回到直播
            </button>
          </>
        )}
        <IconButton icon={volume === 0 ? 'volume-x' : 'volume-2'} label="音量" />
        <div style={{ width: 72, flex: 'none', display: 'flex' }}>
          <Slider value={volume} variant="media" />
        </div>
        <span className="lv-spacer" />
        <PopupButton value={quality} variant="media" />
        <PopupButton value={line} variant="media" />
        <IconButton icon={danmaku ? 'message-square-text' : 'message-square-off'} label={danmaku ? '关闭弹幕' : '打开弹幕'} pressed={danmaku} />
        <IconButton icon="sliders-horizontal" label="弹幕设置" />
        <IconButton icon="picture-in-picture-2" label="画中画" />
        <IconButton icon="maximize" label="全屏" />
      </div>
    </div>
  );
}
