import type { Metadata } from 'next'
import Link from 'next/link'
import { notFound } from 'next/navigation'
import { ArrowDownToLine, ArrowLeft, ArrowRight, ChevronRight, Clock3, Layers3, Search, ShieldCheck, Sparkles } from 'lucide-react'
import {
  catalog,
  getTool,
  toolChannel,
  toolDescription,
  toolName,
  toolStatus,
  type Lang,
} from '@/lib/catalog'
import { HomeProductPreview } from '@/components/HomeProductPreview'
import { ToolCard } from '@/components/ToolCard'
import styles from './detail.module.css'

const copy = {
  zh: {
    home: '首页',
    tools: '全部工具',
    back: '返回全部工具',
    category: '所属分类',
    overview: '工具概览',
    overviewLabel: '为桌面工作流而生',
    info: '工具信息',
    keywords: '搜索关键词',
    status: '发布状态',
    included: '使用方式',
    includedValue: 'ToolForge 桌面应用',
    channelMarket: '在线插件',
    marketIncluded: '应用内工具市场',
    version: '当前版本',
    marketTitle: '在线插件,独立更新',
    marketNote: '这个工具通过应用内工具市场分发:安装一次,更新即时推送,不随应用版本重新下载。',
    marketStepsTitle: '三步,完成在线安装',
    marketOne: '打开 ToolForge',
    marketOneBody: '启动桌面应用,进入「工具市场」。',
    marketTwo: '搜索并安装',
    marketTwoBody: '按名称搜索这个工具,点击安装,几秒内即可使用。',
    marketThree: '打开并开始使用',
    marketThreeBody: '安装后工具会出现在侧边栏,更新时自动提示。',
    desktopNote: '在 ToolForge 桌面应用中使用',
    download: '下载 ToolForge',
    downloadStable: '下载稳定版',
    changelog: '查看发布日志',
    available: '已发布',
    new: '新发布',
    updated: '已更新',
    upcoming: '待发布',
    pendingUpdate: '有待发布更新',
    upcomingTitle: '尚未随稳定版发布',
    upcomingNote: '这个工具已收录在目录中，将在未来版本提供。请以正式发布日志为准。',
    updatingTitle: '包含尚未发布的更新',
    updatingNote: '稳定版已包含此工具的基础功能，最新改动将在后续版本提供。',
    upcomingActionNote: '正式发布后即可在应用中使用',
    updatingActionNote: '稳定版可使用已发布的基础功能',
    visualCaption: '工具信息示意，非应用截图',
    related: '继续探索，同样顺手',
    relatedLabel: '相关工具',
    viewAll: '浏览全部工具',
    stepsTitle: '三步，开始使用',
    futureStepsTitle: '正式发布后，开始使用',
    stepOne: '安装 ToolForge',
    stepOneBody: '选择适合你系统的安装包，下载并打开应用。',
    stepTwo: '找到这个工具',
    stepTwoBody: '在应用内搜索工具名称，或按分类浏览。',
    stepThree: '打开并开始使用',
    stepThreeBody: '按照工具界面提供的操作，完成你的任务。',
    futureOne: '关注正式发布',
    futureOneBody: '查看发布日志，确认该工具已经随稳定版提供。',
    futureTwo: '安装对应版本',
    futureTwoBody: '正式发布后，下载包含此工具的稳定版本。',
    futureThree: '在应用中查找',
    futureThreeBody: '按工具名称或关键词搜索，打开工具开始使用。',
    searchHint: '在应用中搜索',
    noKeywords: '使用工具名称搜索即可。',
  },
  en: {
    home: 'Home',
    tools: 'All tools',
    back: 'Back to all tools',
    category: 'Category',
    overview: 'Tool overview',
    overviewLabel: 'Made for your desktop workflow',
    info: 'Tool information',
    keywords: 'Search keywords',
    status: 'Release status',
    included: 'Available through',
    includedValue: 'ToolForge desktop app',
    channelMarket: 'Online plugin',
    marketIncluded: 'In-app plugin market',
    version: 'Current version',
    marketTitle: 'Online plugin, independent updates',
    marketNote: 'This tool ships through the in-app plugin market: install once, get updates pushed instantly, no app re-download required.',
    marketStepsTitle: 'Install from the market in three steps',
    marketOne: 'Open ToolForge',
    marketOneBody: 'Start the desktop app and open the plugin market.',
    marketTwo: 'Search and install',
    marketTwoBody: 'Search for this tool by name, press install, and it is ready in seconds.',
    marketThree: 'Open it and get to work',
    marketThreeBody: 'Installed tools appear in the sidebar; updates are offered automatically.',
    desktopNote: 'Use it in the ToolForge desktop app',
    download: 'Download ToolForge',
    downloadStable: 'Download stable release',
    changelog: 'View changelog',
    available: 'Released',
    new: 'New release',
    updated: 'Updated',
    upcoming: 'Upcoming',
    pendingUpdate: 'Unreleased updates',
    upcomingTitle: 'Not in a stable release yet',
    upcomingNote: 'This tool is listed in the catalog and is planned for a future release. Check the published changelog for availability.',
    updatingTitle: 'Includes unreleased updates',
    updatingNote: 'The stable app includes the existing functionality. The latest changes will arrive in a future release.',
    upcomingActionNote: 'Available in the app after its official release',
    updatingActionNote: 'Existing functionality is available in the stable app',
    visualCaption: 'Tool information illustration, not an app screenshot',
    related: 'More tools, just as handy',
    relatedLabel: 'Related tools',
    viewAll: 'Browse all tools',
    stepsTitle: 'Get started in three steps',
    futureStepsTitle: 'Get started after release',
    stepOne: 'Install ToolForge',
    stepOneBody: 'Choose the installer for your operating system, then open the app.',
    stepTwo: 'Find this tool',
    stepTwoBody: 'Search for the tool by name or browse its category in the app.',
    stepThree: 'Open it and get to work',
    stepThreeBody: 'Follow the operations provided by the tool to complete your task.',
    futureOne: 'Watch for the release',
    futureOneBody: 'Check the changelog to confirm that the tool has shipped in a stable release.',
    futureTwo: 'Install that version',
    futureTwoBody: 'After release, download a stable app version that includes this tool.',
    futureThree: 'Find it in the app',
    futureThreeBody: 'Search by name or keyword, then open the tool to get started.',
    searchHint: 'Search in the app',
    noKeywords: 'Search by the tool name.',
  },
} satisfies Record<Lang, Record<string, string>>

// Keep every catalog entry available in both languages in the static export.
export function generateStaticParams() {
  return catalog.flatMap((tool) => [
    { lang: 'zh', id: tool.slug },
    { lang: 'en', id: tool.slug },
  ])
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ lang: string; id: string }>
}): Promise<Metadata> {
  const { lang, id } = await params
  const tool = getTool(id)
  if (!tool) return { title: 'ToolForge' }
  const langTag = lang as Lang
  const name = toolName(tool, langTag)
  const title = `${name} · ToolForge`
  const description = toolDescription(tool, langTag)
  const path = `/${langTag}/tools/${tool.slug}/`

  return {
    title,
    description,
    alternates: {
      canonical: path,
      languages: { zh: `/zh/tools/${tool.slug}/`, en: `/en/tools/${tool.slug}/` },
    },
    openGraph: {
      title,
      description,
      type: 'website',
      siteName: 'ToolForge',
      url: `https://toolforge.devlive.org${path}`,
      locale: langTag === 'zh' ? 'zh_CN' : 'en_US',
      images: [{ url: '/og-image.png', width: 1200, height: 630, alt: 'ToolForge' }],
    },
    twitter: { card: 'summary_large_image', title, description, images: ['/og-image.png'] },
  }
}

export default async function ToolDetailPage({
  params,
}: {
  params: Promise<{ lang: string; id: string }>
}) {
  const { lang, id } = await params
  const langTag = lang as Lang
  const tool = getTool(id)
  if (!tool) notFound()

  const t = copy[langTag]
  const name = toolName(tool, langTag)
  const description = toolDescription(tool, langTag)
  const keywords = [...new Set(tool.keywords.filter((keyword) => keyword && keyword.trim() !== ''))]
  const status = toolStatus(tool)
  const isUpcoming = status === 'upcoming'
  const notReleased = isUpcoming && !tool.updateOnly
  // 在线插件：通过应用内工具市场分发，安装/更新步骤与内置工具不同
  const isMarket = toolChannel(tool) === 'market' 
  const statusLabel = tool.updateOnly && isUpcoming
    ? t.pendingUpdate
    : { released: t.available, new: t.new, updated: t.updated, upcoming: t.upcoming }[status]
  const related = [
    ...catalog.filter((entry) => entry.slug !== tool.slug && entry.category === tool.category),
    ...catalog.filter((entry) => entry.slug !== tool.slug && entry.category !== tool.category),
  ].slice(0, 3)
  const steps = notReleased
    ? [
        { title: t.futureOne, body: t.futureOneBody },
        { title: t.futureTwo, body: t.futureTwoBody },
        { title: t.futureThree, body: t.futureThreeBody },
      ]
    : isMarket
      ? [
          { title: t.marketOne, body: t.marketOneBody },
          { title: t.marketTwo, body: t.marketTwoBody },
          { title: t.marketThree, body: t.marketThreeBody },
        ]
      : [
          { title: t.stepOne, body: t.stepOneBody },
          { title: t.stepTwo, body: t.stepTwoBody },
          { title: t.stepThree, body: t.stepThreeBody },
        ]

  return (
    <article className={styles.detail}>
      <div className={styles.wrap}>
        <nav className={styles.breadcrumb} aria-label={langTag === 'zh' ? '面包屑导航' : 'Breadcrumb'}>
          <Link href={`/${langTag}/`}>{t.home}</Link>
          <ChevronRight size={12} aria-hidden="true" />
          <Link href={`/${langTag}/tools/`}>{t.tools}</Link>
          <ChevronRight size={12} aria-hidden="true" />
          <span aria-current="page">{name}</span>
        </nav>

        <header className={styles.hero}>
          <div className={styles.heroMain}>
            <span className={styles.heroIcon}><span className={styles.heroGlyph} aria-hidden="true" style={{ maskImage: `url(/icons/${tool.slug}.svg)`, WebkitMaskImage: `url(/icons/${tool.slug}.svg)` }} /></span>
            <div className={styles.heroCopy}>
              <div className={styles.labels}>
                <span className={styles.category}>{tool.categoryLabel[langTag]}</span>
                {isMarket && <span className={styles.market}>{t.channelMarket}</span>}
                <span className={`${styles.status} ${isUpcoming ? styles.pending : ''}`}>
                  <span aria-hidden="true" />{statusLabel}
                </span>
              </div>
              <h1>{name}</h1>
              <p>{description}</p>
            </div>
          </div>
          <div className={styles.heroActions}>
            <Link href={notReleased ? `/${langTag}/changelog/` : `/${langTag}/#download`} className={styles.primary}>
              {notReleased ? <Clock3 size={16} aria-hidden="true" /> : <ArrowDownToLine size={16} aria-hidden="true" />}
              {notReleased ? t.changelog : tool.updateOnly && isUpcoming ? t.downloadStable : t.download}
              <ArrowRight size={14} aria-hidden="true" />
            </Link>
            <Link href={`/${langTag}/tools/`} className={styles.back}>
              <ArrowLeft size={13} aria-hidden="true" />{t.back}
            </Link>
            <p>{notReleased ? t.upcomingActionNote : tool.updateOnly && isUpcoming ? t.updatingActionNote : isMarket ? t.marketNote : t.desktopNote}</p>
          </div>
        </header>

        {isUpcoming && (
          <aside className={styles.notice}>
            <Clock3 size={18} aria-hidden="true" />
            <div>
              <h2>{tool.updateOnly ? t.updatingTitle : t.upcomingTitle}</h2>
              <p>{tool.updateOnly ? t.updatingNote : t.upcomingNote}</p>
            </div>
            <Link href={`/${langTag}/changelog/`}>{t.changelog}<ArrowRight size={13} aria-hidden="true" /></Link>
          </aside>
        )}

        {isMarket && !isUpcoming && (
          <aside className={styles.notice}>
            <ShieldCheck size={18} aria-hidden="true" />
            <div>
              <h2>{t.marketTitle}</h2>
              <p>{t.marketNote}</p>
            </div>
            <Link href={`/${langTag}/tools/`}>{t.viewAll}<ArrowRight size={13} aria-hidden="true" /></Link>
          </aside>
        )}

        <div className={styles.contentGrid}>
          <div className={styles.mainColumn}>
            <section className={styles.overview} aria-labelledby="tool-overview">
              <div className={styles.sectionLabel}><Layers3 size={14} aria-hidden="true" /><h2 id="tool-overview">{t.overview}</h2></div>
              {tool.slug === 'json-formatter' ? (
                <div className={styles.appPreview}>
                  <HomeProductPreview lang={langTag} toolCount={catalog.length} />
                </div>
              ) : (
                <figure className={styles.toolVisual}>
                  <div className={styles.visualTop}>
                    <span><img src="/icon.png" alt="" width={17} height={17} />ToolForge</span>
                    <span>{tool.categoryLabel[langTag]}</span>
                  </div>
                  <div className={styles.visualCenter}>
                    <span className={styles.visualIcon}><span className={styles.visualGlyph} aria-hidden="true" style={{ maskImage: `url(/icons/${tool.slug}.svg)`, WebkitMaskImage: `url(/icons/${tool.slug}.svg)` }} /></span>
                    <p className={styles.visualEyebrow}>{t.overviewLabel}</p>
                    <p className={styles.visualName}>{name}</p>
                    <div className={styles.visualKeywords}>
                      {keywords.slice(0, 4).map((keyword) => <span key={keyword}>{keyword}</span>)}
                    </div>
                  </div>
                  <figcaption>{t.visualCaption}</figcaption>
                </figure>
              )}
            </section>

            <section className={styles.howTo} aria-labelledby="tool-steps">
              <div className={styles.sectionLabel}><Sparkles size={14} aria-hidden="true" /><h2 id="tool-steps">{notReleased ? t.futureStepsTitle : isMarket ? t.marketStepsTitle : t.stepsTitle}</h2></div>
              <ol className={styles.steps}>
                {steps.map((step, index) => (
                  <li key={step.title}>
                    <span className={styles.stepNumber}>{String(index + 1).padStart(2, '0')}</span>
                    <div><h3>{step.title}</h3><p>{step.body}</p></div>
                  </li>
                ))}
              </ol>
              <div className={styles.searchHint}><Search size={14} aria-hidden="true" /><span>{t.searchHint}</span><strong>{name}</strong></div>
            </section>
          </div>

          <aside className={styles.sidebar}>
            <section className={styles.infoPanel} aria-labelledby="tool-info">
              <h2 id="tool-info">{t.info}</h2>
              <dl>
                <div><dt>{t.category}</dt><dd>{tool.categoryLabel[langTag]}</dd></div>
                <div><dt>{t.status}</dt><dd className={isUpcoming ? styles.pendingText : styles.accentText}>{statusLabel}</dd></div>
                <div><dt>{t.included}</dt><dd>{isMarket ? t.marketIncluded : t.includedValue}</dd></div>
                {tool.version && <div><dt>{t.version}</dt><dd>{tool.version}</dd></div>}
              </dl>
              <Link href={`/${langTag}/changelog/`} className={styles.infoLink}>{t.changelog}<ArrowRight size={13} aria-hidden="true" /></Link>
            </section>
            <section className={styles.keywordPanel} aria-labelledby="tool-keywords">
              <h2 id="tool-keywords">{t.keywords}</h2>
              {keywords.length > 0 ? (
                <div className={styles.keywords}>{keywords.map((keyword) => <span key={keyword}>{keyword}</span>)}</div>
              ) : <p>{t.noKeywords}</p>}
            </section>
          </aside>
        </div>

        <section className={styles.related} aria-labelledby="related-tools">
          <div className={styles.relatedHeading}>
            <div><p>{t.relatedLabel}</p><h2 id="related-tools">{t.related}</h2></div>
            <Link href={`/${langTag}/tools/`}>{t.viewAll}<ArrowRight size={14} aria-hidden="true" /></Link>
          </div>
          <div className={styles.relatedGrid}>{related.map((entry) => <ToolCard key={entry.slug} tool={entry} lang={langTag} />)}</div>
        </section>
      </div>
    </article>
  )
}
