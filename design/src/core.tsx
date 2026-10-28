import type { CSSProperties, ReactNode } from 'react';
import { ICONS, type IconName } from './icons';

export type { IconName } from './icons';
export type Platform = 'bilibili' | 'douyu' | 'huya';
export const PLATFORM_NAMES: Record<Platform, string> = { bilibili: '哔哩哔哩', douyu: '斗鱼', huya: '虎牙' };

export interface LivoProviderProps {
  /** 主题：dark（默认）或 light。所有 LIVO 组件都要放在 LivoProvider 里才有令牌和字体。 */
  theme?: 'dark' | 'light';
  children?: ReactNode;
  className?: string;
  style?: CSSProperties;
}

/** LIVO 的根容器：设置 data-theme 与 lv-root（字体、正文字号、背景色），所有组件都放在它里面。 */
export function LivoProvider({ theme = 'dark', children, className, style }: LivoProviderProps) {
  return (
    <div className={['lv-root', className].filter(Boolean).join(' ')} data-theme={theme} style={style}>
      {children}
    </div>
  );
}

export interface IconProps {
  /** 图标名，只能用设计系统 Iconography 清单里的 Lucide 图标 */
  name: IconName;
  /** 16（列表、按钮、菜单）或 20（工具栏、播放器） */
  size?: 16 | 20;
  /** 无障碍标签；不传则视为装饰图标 */
  label?: string;
  className?: string;
}

/** Lucide 1.63.0 线性图标，跟随文字颜色（currentColor），保持 24 网格 2 线宽。 */
export function Icon({ name, size = 16, label, className }: IconProps) {
  const cls = ['lv-icon', size === 20 ? 'lv-icon-20' : '', className].filter(Boolean).join(' ');
  return (
    <svg
      className={cls}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      dangerouslySetInnerHTML={{ __html: ICONS[name] }}
    />
  );
}

// 标识几何：取自 Figma 导出的 LIVO Brand Kit，坐标原样保留
const V = '#6E58F5', I = '#6B7CF0', C = '#4BD3CF', INK = '#171A23';
const L_PATH = 'M0 0H48.8727V175.127C48.8727 197.811 57.8836 219.565 73.9231 235.604C89.9626 251.644 111.717 260.655 134.4 260.655H198.4V309.527H134.4C98.7549 309.527 64.5697 295.367 39.3648 270.162C14.16 244.958 0 210.772 0 175.127V0Z';
const S1 = 'M134 132.611C148.587 132.611 162.576 138.341 172.891 148.541C183.205 158.741 189 172.575 189 187H156C156 181.23 153.682 175.696 149.556 171.617C145.431 167.537 139.835 165.244 134 165.244V132.611Z';
const S2 = 'M134 68.157C165.873 68.157 196.441 80.6779 218.979 102.965C241.517 125.253 254.178 155.481 254.178 187H220.022C220.022 164.439 210.959 142.802 194.826 126.849C178.694 110.896 156.814 101.934 134 101.934V68.157Z';
const S3 = 'M134 0C184.126 0 232.199 19.6912 267.643 54.7418C303.088 89.7923 323 137.331 323 186.9H285.2C285.2 147.245 269.27 109.214 240.915 81.1734C212.559 53.133 174.101 37.38 134 37.38V0Z';
const L_SMALL = 'M0 0H64V175.127A70.4 70.4 0 0 0 134.4 245.527H198.4V309.527H134.4A134.4 134.4 0 0 1 0 175.127V0Z';
const S1_SMALL = 'M134 121A66 66 0 0 1 200 187H134Z';
const S3_SMALL = 'M134 0A189 187 0 0 1 323 187H256A122 120 0 0 0 134 67Z';
const WORD = [
  'M0 187H54V485H264V535H0V187Z',
  'M318 187H372V535H318V187Z',
  'M426 187H486L600 472L714 187H774L624 535H576L426 187Z',
];
const O_PATH = 'M966 183C1065.41 183 1146 262.693 1146 361C1146 459.307 1065.41 539 966 539C866.589 539 786 459.307 786 361C786 262.693 866.589 183 966 183ZM966 233C1034.48 233 1090 290.308 1090 361C1090 431.692 1034.48 489 966 489C897.517 489 842 431.692 842 361C842 290.308 897.517 233 966 233Z';

export interface LogoProps {
  /** mark 符号；mark-small 16–23px 用的小尺寸版；lockup 组合标识；signal 信号弧辅助图形；app-icon App 图标 */
  variant?: 'mark' | 'mark-small' | 'lockup' | 'signal' | 'app-icon';
  /** color 标准彩色；reverse 深底反白（字标白、弧彩色）；black / white 单色 */
  tone?: 'color' | 'reverse' | 'black' | 'white';
  /** app-icon 的底板：light（默认）或 dark */
  appearance?: 'light' | 'dark';
  /** 显示高度（px）；宽度按比例 */
  height?: number;
  className?: string;
}

/** LIVO 标识：符号、组合标识、信号弧与 App 图标，按 Figma 原始几何绘制。只等比缩放，不改色、不重排。 */
export function Logo({ variant = 'mark', tone = 'color', appearance = 'light', height = 32, className }: LogoProps) {
  const mono = tone === 'black' ? '#000000' : tone === 'white' ? '#FFFFFF' : null;
  const cl = mono ?? V, c1 = mono ?? V, c2 = mono ?? I, c3 = mono ?? C;
  const word = mono ?? (tone === 'reverse' ? '#FFFFFF' : INK);
  const cls = ['lv-logo', className].filter(Boolean).join(' ');
  if (variant === 'lockup') {
    const w = (height * 1321) / 539;
    return (
      <svg className={cls} width={w} height={height} viewBox="0 0 1321 539" role="img" aria-label="LIVO">
        {WORD.map((d) => <path key={d} d={d} fill={word} />)}
        <path fillRule="evenodd" clipRule="evenodd" d={O_PATH} fill={word} />
        <g transform="translate(998 0)">
          <path d={S1} fill={c1} />
          <path d={S2} fill={c2} />
          <path d={S3} fill={c3} />
        </g>
      </svg>
    );
  }
  if (variant === 'signal') {
    return (
      <svg className={cls} width={(height * 189) / 187} height={height} viewBox="134 0 189 187" role="img" aria-label="LIVO 信号">
        <path d={S1} fill={c1} />
        <path d={S2} fill={c2} />
        <path d={S3} fill={c3} />
      </svg>
    );
  }
  if (variant === 'app-icon') {
    const dark = appearance === 'dark';
    const k = 824 / 512;
    return (
      <svg className={cls} width={height} height={height} viewBox="0 0 1024 1024" role="img" aria-label="LIVO App 图标">
        <rect x="100" y="100" width="824" height="824" rx="185.4" fill={dark ? '#1F2234' : '#F8F9FD'} />
        <rect x="101" y="101" width="822" height="822" rx="184.4" fill="none" stroke={dark ? '#353A54' : '#E2E5F0'} strokeWidth="2" />
        <g transform={`translate(${100 + 96 * k} ${100 + 101 * k}) scale(${k})`}>
          <path d={L_PATH} fill={V} />
          <path d={S1} fill={V} />
          <path d={S2} fill={I} />
          <path d={S3} fill={C} />
        </g>
      </svg>
    );
  }
  const small = variant === 'mark-small';
  return (
    <svg className={cls} width={(height * 323) / 309.527} height={height} viewBox="0 0 323 309.527" role="img" aria-label="LIVO">
      <path d={small ? L_SMALL : L_PATH} fill={cl} />
      <path d={small ? S1_SMALL : S1} fill={c1} />
      {!small && <path d={S2} fill={c2} />}
      <path d={small ? S3_SMALL : S3} fill={c3} />
    </svg>
  );
}
