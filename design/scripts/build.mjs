// 构建 @livo/design：tokens.json → tokens.css；拼 dist/livo.css；esbuild 打 JS；tsc 出类型。
import { readFileSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { build } from 'esbuild';

const root = new URL('..', import.meta.url).pathname;
const src = (p) => root + 'src/' + p;
const dist = (p) => root + 'dist/' + p;

rmSync(root + 'dist', { recursive: true, force: true });
mkdirSync(root + 'dist', { recursive: true });

// tokens.css：第一个主题（深色）写在 :root，其余主题用 [data-theme]
const t = JSON.parse(readFileSync(src('tokens.json'), 'utf8'));
const themes = t.color.themes.map((x) => x.id);
const byName = Object.fromEntries(t.color.tokens.map((c) => [c.name, c]));
const pick = (v, th) => (typeof v === 'string' ? v : v[th] ?? v[themes[0]]);
const ref = (v) => {
  const m = /^\{(.+)\}$/.exec(v);
  return m ? `var(--${m[1]})` : v;
};
const block = (sel, th) => {
  const lines = t.color.tokens.map((c) => `  --${c.name}: ${ref(pick(c.value, th))};`);
  for (const s of t.shadow.tokens) lines.push(`  --${s.name}: ${pick(s.value, th)};`);
  return `${sel} {\n${lines.join('\n')}\n}`;
};
const out = ['/* LIVO 设计系统 — generated from tokens.json */'];
themes.forEach((th, i) => out.push(block(i === 0 ? `:root, [data-theme="${th}"]` : `[data-theme="${th}"]`, th)));
const scalars = [];
for (const fam of ['spacing', 'radius', 'size']) for (const c of t[fam].tokens) scalars.push(`  --${c.name}: ${c.value};`);
for (const [k, v] of Object.entries(t.type.families)) scalars.push(`  --font-${k}: ${v};`);
out.push(`:root {\n${scalars.join('\n')}\n}`);
// 每个文字样式一个 lv-text-<name> 类
for (const g of t.type.groups) {
  for (const s of g.styles) {
    const fam = s.family ?? g.family;
    const decl = [`font-family: var(--font-${fam})`, `font-size: ${s.fontSize}`, `line-height: ${s.lineHeight}`, `font-weight: ${s.fontWeight}`];
    if (s.letterSpacing) decl.push(`letter-spacing: ${s.letterSpacing}`);
    out.push(`.lv-text-${s.name} { ${decl.join('; ')}; }`);
  }
}
writeFileSync(dist('tokens.css'), out.join('\n\n') + '\n');

// livo.css：品牌字体（Google Fonts）+ 令牌 + 组件样式
const fonts = '@import url("https://fonts.googleapis.com/css2?family=Outfit:wght@400;600;700&family=Noto+Sans+SC:wght@400;700&display=swap");\n';
writeFileSync(dist('livo.css'), fonts + readFileSync(dist('tokens.css'), 'utf8') + '\n' + readFileSync(src('styles/components.css'), 'utf8') + '\n' + readFileSync(src('styles/extra.css'), 'utf8'));

await build({
  entryPoints: [src('index.ts')],
  outfile: dist('index.js'),
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2019',
  jsx: 'automatic',
  external: ['react', 'react-dom', 'react/jsx-runtime'],
  logLevel: 'warning',
});

execFileSync(root + 'node_modules/.bin/tsc', ['-p', root + 'tsconfig.json'], { stdio: 'inherit' });
console.log('built dist/');
