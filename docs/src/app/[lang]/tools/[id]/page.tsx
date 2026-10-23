import Link from 'next/link'
import { notFound } from 'next/navigation'
import { ArrowRight, CheckCircle2, ChevronRight, Clock3 } from 'lucide-react'
import { DownloadDialog } from '@/components/DownloadDialog'
import { InstallButton } from '@/components/InstallButton'
import { DetailTabs } from '@/components/DetailTabs'
import { ToolCard, ToolIcon } from '@/components/ToolCard'
import { ToolPermissions } from '@/components/ToolPermissions'
import { StatusBadge } from '@/components/StatusBadge'
import { canInstall, catalog, getTool, toolChannel, toolDescription, toolName, toolStatus, type Lang, type ToolEntry } from '@/lib/catalog'
import { getToolDocumentation } from '@/lib/tool-docs'
import { pageMetadata } from '@/lib/metadata'
import { siteCopy } from '@/lib/site-copy'

export function generateStaticParams() {
  return catalog.flatMap((tool) => [
    { lang: 'zh', id: tool.slug },
    { lang: 'en', id: tool.slug },
  ])
}
export async function generateMetadata({ params }: { params: Promise<{ lang: Lang; id: string }> }) {
  const { lang, id } = await params
  const tool = getTool(id)
  return tool ? pageMetadata(lang, `tools/${id}/`, `${toolName(tool, lang)} — ToolForge`, toolDescription(tool, lang)) : { title: 'ToolForge' }
}
function Preview({ tool, lang }: { tool: ToolEntry; lang: Lang }) {
  const zh = lang === 'zh'
  return tool.slug === 'json-formatter' ? (
    <figure className="site-preview">
      <figcaption className="site-preview-bar">
        <span>{toolName(tool, lang)}</span>
        <span className="site-tag">{zh ? '操作示意' : 'Illustration'}</span>
      </figcaption>
      <div className="site-editors">
        <div className="site-editor">
          <p>{zh ? '输入' : 'Input'}</p>
          <pre>{'{"name":"ToolForge","local":true,"hello":"world"}'}</pre>
        </div>
        <div className="site-editor">
          <p>{zh ? '格式化结果' : 'Formatted result'}</p>
          <pre>
            {'{\n  '}
            <span className="site-code-key">{'"name"'}</span>
            {': '}
            <span className="site-code-value">{'"ToolForge"'}</span>
            {',\n  '}
            <span className="site-code-key">{'"local"'}</span>
            {': true,\n  '}
            <span className="site-code-key">{'"hello"'}</span>
            {': '}
            <span className="site-code-value">{'"world"'}</span>
            {'\n}'}
          </pre>
        </div>
      </div>
      <div className="site-preview-footer">
        <CheckCircle2 aria-hidden="true" />
        {zh ? 'JSON 有效' : 'Valid JSON'}
        <span className="ml-auto">UTF-8 · {zh ? '2 空格' : '2 spaces'}</span>
      </div>
    </figure>
  ) : (
    <div className="site-keywords" aria-label={zh ? '搜索关键词' : 'Search keywords'}>
      {[...new Set(tool.keywords)].slice(0, 8).map((keyword) => (
        <span className="site-tag" key={keyword}>
          {keyword}
        </span>
      ))}
    </div>
  )
}
export default async function ToolDetailPage({ params }: { params: Promise<{ lang: Lang; id: string }> }) {
  const { lang, id } = await params
  const tool = getTool(id)
  if (!tool) notFound()
  const zh = lang === 'zh'
  const t = siteCopy[lang]
  const data = getToolDocumentation(tool.slug)
  const market = toolChannel(tool) === 'market'
  const pending = toolStatus(tool) === 'upcoming'
  const unpublished = pending && !tool.updateOnly
  const help = data.help[lang]
  const related = catalog.filter((entry) => entry.slug !== tool.slug && entry.category === tool.category).slice(0, 3)
  const overview = (
    <div className="site-prose">
      <h2>
        {tool.slug === 'json-formatter'
          ? zh
            ? '让结构清楚，让调试顺手。'
            : 'Clear structure. Easier debugging.'
          : zh
            ? '它能帮你做什么？'
            : 'What can this tool do?'}
      </h2>
      <p>{help[0] || toolDescription(tool, lang)}</p>
      <Preview tool={tool} lang={lang} />
      <h3>{zh ? '开始使用前' : 'Before you begin'}</h3>
      <p>
        {unpublished
          ? zh
            ? '正式发布后，在线插件可从工具市场安装。当前可以先了解用法与输入范围。'
            : 'After release, this plugin can be installed from the tool market. You can read its usage and input limits in the meantime.'
          : tool.builtinFallback
            ? zh
              ? '稳定版中仍可找到内置基础功能。当前代码已迁到在线插件，在线包发布后再按需安装更新。'
              : 'The stable app still provides the basic built-in tool. The current code has moved to an online plugin; install it once the package is published.'
            : market
              ? zh
                ? '通过应用内工具市场安装，插件可独立更新。安装前先查看所需权限，再确认操作。'
                : 'Install through the in-app tool market. Plugins update independently; review their permissions before installing.'
              : zh
                ? '这是内置工具。安装适合你系统的 ToolForge 版本，在侧栏中搜索工具名称即可找到。'
                : 'This tool is built in. Install ToolForge for your system, then search for its name in the sidebar.'}
      </p>
      <div className="site-note">
        {zh
          ? '网页用于了解工具和阅读文档。实际处理在 ToolForge 桌面应用中完成。'
          : 'This website helps you discover tools and read their guides. Processing takes place in the ToolForge desktop app.'}
      </div>
    </div>
  )
  const usage = (
    <div className="site-prose">
      <h2>{zh ? '使用方法与边界' : 'Usage and limits'}</h2>
      {help.length > 0 ? (
        <ol className="site-help-list">
          {help.map((text, index) => (
            <li key={index}>{text}</li>
          ))}
        </ol>
      ) : (
        <p>
          {zh
            ? '此工具尚未提供独立的使用说明。请先在应用内查看输入提示和帮助。'
            : 'A standalone usage guide is not available yet. Check the input hints and help in the app.'}
        </p>
      )}
      {tool.slug === 'json-formatter' && <Preview tool={tool} lang={lang} />}
      <h3>{zh ? '需要安装帮助？' : 'Need installation help?'}</h3>
      <p>
        <Link className="site-link" href={`/${lang}/guide/${market ? 'plugins' : 'start'}/`}>
          {market ? t.guidePlugins : t.guideStart}
          <ArrowRight aria-hidden="true" />
        </Link>
      </p>
    </div>
  )
  return (
    <article className="site-wrap">
      <header className="site-page-heading">
        <nav className="site-breadcrumb" aria-label={zh ? '面包屑导航' : 'Breadcrumb'}>
          <Link href={`/${lang}/tools/`}>{t.tools}</Link>
          <ChevronRight aria-hidden="true" />
          <span>{tool.categoryLabel[lang]}</span>
          <ChevronRight aria-hidden="true" />
          <span aria-current="page">{toolName(tool, lang)}</span>
        </nav>
        <div className="site-detail-head">
          <div>
            <div className="site-detail-title">
              <ToolIcon slug={tool.slug} hasIcon={tool.hasIcon} />
              <div>
                <h1>{toolName(tool, lang)}</h1>
                <div className="site-detail-meta">
                  <span className="site-tag">{market ? (zh ? '在线插件' : 'Online plugin') : zh ? '应用内置' : 'Built-in'}</span>
                  <span>{tool.categoryLabel[lang]}</span>
                  <span>
                    {zh ? '文档版本' : 'Documented version'} {tool.version}
                  </span>
                  <StatusBadge tool={tool} lang={lang} />
                  {data.platforms.length > 0 && <span>{data.platforms.join(' / ')}</span>}
                </div>
              </div>
            </div>
            <p className="site-detail-summary">{toolDescription(tool, lang)}</p>
          </div>
          <div className="site-detail-actions">
            {canInstall(tool) ? (
              <InstallButton slug={tool.slug} lang={lang} variant="primary" />
            ) : unpublished ? (
              <Link href={`/${lang}/changelog/`} className="site-secondary">
                <Clock3 aria-hidden="true" />
                {zh ? '关注正式发布' : 'Watch for the release'}
              </Link>
            ) : (
              <DownloadDialog lang={lang} />
            )}
          </div>
        </div>
      </header>
      {pending && (
        <div className="site-note site-note-warn">
          {tool.builtinFallback || (!market && tool.updateOnly)
            ? zh
              ? '稳定版应用已包含此工具的基础功能。本文档对应尚未发布的更新；若涉及在线迁移，请等待在线包正式发布。'
              : 'The stable app already includes the basic built-in tool. This guide covers unreleased changes; an online migration still requires a published plugin package.'
            : tool.updateOnly
              ? zh
                ? `已发布版本 ${tool.publishedVersion ?? ''} 可用；本文档包含尚未发布的更新，请以实际安装版本为准。`
                : `Published version ${tool.publishedVersion ?? ''} is available. This guide includes unreleased changes; check your installed version.`
              : zh
                ? '此工具尚未确认发布，当前仅提供功能与用法说明。'
                : 'This tool is not yet confirmed as released. Its features and usage are documented here.'}
        </div>
      )}
      {market && tool.releaseVerified === false && (
        <div className="site-note site-note-warn">
          {zh
            ? '暂时无法核对在线插件的发布信息。你可以在应用内市场确认可用版本。'
            : 'Online release details could not be verified. Check the in-app market for availability.'}
        </div>
      )}
      <div className="site-detail-body">
        <div>
          <DetailTabs
            label={zh ? '工具文档' : 'Tool documentation'}
            panels={[
              { id: 'overview', title: zh ? '工具概览' : 'Overview', content: overview },
              { id: 'usage', title: zh ? '使用方法' : 'Usage', content: usage },
              { id: 'permissions', title: zh ? '权限与 Agent' : 'Permissions & Agent', content: <ToolPermissions data={data} lang={lang} /> },
            ]}
          />
        </div>
        <aside className="site-info">
          <h3>{zh ? '工具信息' : 'Tool information'}</h3>
          {[
            [zh ? '使用方式' : 'Use through', zh ? '桌面应用' : 'Desktop app'],
            [zh ? '安装来源' : 'Source', market ? (zh ? '工具市场' : 'Tool market') : zh ? '应用内置' : 'Built-in'],
            [zh ? '已发布插件版本' : 'Published plugin', tool.publishedVersion ?? '—'],
            [zh ? '最低宿主版本' : 'Minimum app', data.minAppVersion || '—'],
            [zh ? '系统权限' : 'Permissions', String(data.permissions.length)],
          ].map(([label, value]) => (
            <div className="site-info-row" key={label}>
              <span>{label}</span>
              <span>{value}</span>
            </div>
          ))}
          <p className="site-info-help">
            {zh ? '查看输入说明与所需权限，再选择适合你的使用方式。' : 'Review the inputs and permissions, then choose how to use the tool.'}
          </p>
          <Link className="site-link" href={`/${lang}/guide/start/`}>
            {t.guideStart}
            <ArrowRight aria-hidden="true" />
          </Link>
        </aside>
      </div>
      {related.length > 0 && (
        <section className="site-tool-related site-section">
          <div className="site-section-head">
            <h2>{zh ? '继续探索，同样顺手' : 'More tools, just as useful'}</h2>
            <Link className="site-link" href={`/${lang}/tools/`}>
              {t.viewAll}
              <ArrowRight aria-hidden="true" />
            </Link>
          </div>
          <div className="site-grid">
            {related.map((item) => (
              <ToolCard key={item.slug} tool={item} lang={lang} />
            ))}
          </div>
        </section>
      )}
    </article>
  )
}
