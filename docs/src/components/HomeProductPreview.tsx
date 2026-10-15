import {
  ArrowRight,
  Braces,
  Check,
  ChevronDown,
  Code2,
  Command,
  Globe2,
  History,
  Image,
  Maximize2,
  MoreHorizontal,
  Search,
  ShieldCheck,
  Star,
  Type,
} from 'lucide-react'
import type { Lang } from '@/lib/catalog'
import styles from './HomeProductPreview.module.css'

const copy = {
  zh: {
    illustration: '应用界面示意',
    accessible: 'ToolForge 桌面应用概念界面，展示本地 JSON 格式化与工具搜索',
    search: '搜索工具',
    recent: '最近使用',
    favorites: '我的收藏',
    categories: '工具分类',
    development: '开发',
    images: '图像',
    text: '文本',
    network: '网络',
    formatter: 'JSON 格式化',
    query: 'JSON 查询',
    local: '本地处理',
    input: '输入',
    output: '格式化结果',
    format: '格式化',
    valid: 'JSON 有效',
    spaces: '2 空格',
    tools: '目录收录 {count} 个工具',
    find: '工具快速搜索',
    select: '选择',
    open: '打开',
    private: '数据留在你的设备',
  },
  en: {
    illustration: 'Illustrative app preview',
    accessible: 'ToolForge desktop app concept showing local JSON formatting and tool search',
    search: 'Search tools',
    recent: 'Recently used',
    favorites: 'Favorites',
    categories: 'CATEGORIES',
    development: 'Development',
    images: 'Images',
    text: 'Text',
    network: 'Network',
    formatter: 'JSON Formatter',
    query: 'JSON Query',
    local: 'Local processing',
    input: 'Input',
    output: 'Formatted',
    format: 'Format',
    valid: 'Valid JSON',
    spaces: '2 spaces',
    tools: '{count} tools in the catalog',
    find: 'Quick tool search',
    select: 'Select',
    open: 'Open',
    private: 'Your data stays on your device',
  },
} satisfies Record<Lang, Record<string, string>>

export function HomeProductPreview({ lang, toolCount }: { lang: Lang; toolCount: number }) {
  const t = copy[lang]
  const categories = [
    { icon: Code2, label: t.development, active: true },
    { icon: Image, label: t.images, active: false },
    { icon: Type, label: t.text, active: false },
    { icon: Globe2, label: t.network, active: false },
  ]

  return (
    <figure className={styles.preview} aria-label={t.accessible}>
      <div className={styles.glow} aria-hidden="true" />
      <div className={styles.window}>
        <div className={styles.titlebar}>
          <span className={styles.trafficLights} aria-hidden="true">
            <i />
            <i />
            <i />
          </span>
          <span className={styles.appTitle}>
            <img src="/icon.png" alt="" width={17} height={17} />
            ToolForge
          </span>
          <MoreHorizontal size={16} aria-hidden="true" />
        </div>

        <div className={styles.appBody}>
          <aside className={styles.sidebar} aria-label={t.categories}>
            <span className={styles.searchField}>
              <Search size={12} aria-hidden="true" />
              <span>{t.search}</span>
              <Command size={10} aria-hidden="true" />
              <span className={styles.shortcut}>K</span>
            </span>
            <div className={styles.quickLinks}>
              <span><History size={14} aria-hidden="true" />{t.recent}</span>
              <span><Star size={14} aria-hidden="true" />{t.favorites}</span>
            </div>
            <p className={styles.categoryLabel}>{t.categories}</p>
            <div className={styles.categories}>
              {categories.map(({ icon: Icon, label, active }) => (
                <span key={label} className={active ? styles.activeCategory : undefined}>
                  <Icon size={14} aria-hidden="true" />
                  {label}
                  {active && <span className={styles.activeDot} />}
                </span>
              ))}
            </div>
            <div className={styles.sidebarBottom}>
              <span className={styles.liveDot} />
              <span>{t.tools.replace('{count}', String(toolCount))}</span>
            </div>
          </aside>

          <div className={styles.workspace}>
            <div className={styles.toolHeader}>
              <span className={styles.toolIcon}><Braces size={18} aria-hidden="true" /></span>
              <span className={styles.toolName}>{t.formatter}</span>
              <Star className={styles.favoriteIcon} size={13} aria-hidden="true" />
              <span className={styles.localBadge}><ShieldCheck size={10} aria-hidden="true" />{t.local}</span>
            </div>
            <div className={styles.editorToolbar}>
              <span className={styles.formatAction}><Braces size={12} aria-hidden="true" />{t.format}<ArrowRight size={11} aria-hidden="true" /></span>
              <span className={styles.indent}>{t.spaces}<ChevronDown size={10} aria-hidden="true" /></span>
            </div>
            <div className={styles.editors}>
              <div className={styles.editor}>
                <div className={styles.editorLabel}><span>{t.input}</span><MoreHorizontal size={12} aria-hidden="true" /></div>
                <div className={styles.compactCode}>
                  <span className={styles.lineNumber}>1</span>
                  <code>{'{'}<span className={styles.key}>{'"name"'}</span>:<span className={styles.string}>{'"ToolForge"'}</span>,<span className={styles.key}>{'"local"'}</span>:<span className={styles.boolean}>true</span>,<span className={styles.key}>{'"tools"'}</span>:<span className={styles.number}>{toolCount}</span>{'}'}</code>
                </div>
              </div>
              <div className={`${styles.editor} ${styles.outputEditor}`}>
                <div className={styles.editorLabel}><span>{t.output}</span><Maximize2 size={10} aria-hidden="true" /></div>
                <div className={styles.formattedCode}>
                  <div><span className={styles.lineNumber}>1</span><code>{'{'}</code></div>
                  <div><span className={styles.lineNumber}>2</span><code>{'  '}<span className={styles.key}>{'"name"'}</span>: <span className={styles.string}>{'"ToolForge"'}</span>,</code></div>
                  <div><span className={styles.lineNumber}>3</span><code>{'  '}<span className={styles.key}>{'"local"'}</span>: <span className={styles.boolean}>true</span>,</code></div>
                  <div><span className={styles.lineNumber}>4</span><code>{'  '}<span className={styles.key}>{'"tools"'}</span>: <span className={styles.number}>{toolCount}</span></code></div>
                  <div><span className={styles.lineNumber}>5</span><code>{'}'}</code></div>
                </div>
              </div>
            </div>
            <div className={styles.statusbar}>
              <span><Check size={11} aria-hidden="true" />{t.valid}</span>
              <span>UTF-8</span>
            </div>
          </div>
        </div>
      </div>

      <div className={styles.palette} aria-label={t.find}>
        <div className={styles.paletteInput}><Search size={15} aria-hidden="true" /><span>json</span><span className={styles.caret} /><span className={styles.escape}>esc</span></div>
        <div className={styles.paletteResults}>
          <span className={styles.selectedResult}><Braces size={15} aria-hidden="true" /><span>{t.formatter}</span><span className={styles.enter}>↵</span></span>
          <span><Code2 size={15} aria-hidden="true" /><span>{t.query}</span></span>
        </div>
        <div className={styles.paletteFooter}><span><span>↑ ↓</span> {t.select}</span><span><span>↵</span> {t.open}</span></div>
      </div>

      <figcaption className={styles.caption}><span>{t.illustration}</span><span><ShieldCheck size={11} aria-hidden="true" />{t.private}</span></figcaption>
    </figure>
  )
}
