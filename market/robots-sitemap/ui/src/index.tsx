import { useState } from 'react'
import { Button, Input, Panel, SegmentedControl, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Bot, Copy } from 'lucide-react'

interface RobotsOut {
  text: string
  groups: number
}
interface SitemapOut {
  xml: string
  shards: number
  warnings: string[]
}

export default function RobotsSitemap() {
  const { t, errorMessage } = usePlugin()
  const [tab, setTab] = useState<'robots' | 'sitemap'>('robots')
  const [ua, setUa] = useState('*') // tf-allow: 默认值
  const [rules, setRules] = useState('disallow:/admin\nallow:/') // tf-allow: 默认值
  const [urls, setUrls] = useState('https://example.com/\nhttps://example.com/posts') // tf-allow: 默认值
  const [changefreq, setChangefreq] = useState('weekly') // tf-allow: 默认值

  const ruleList = rules.split('\n').map((rule) => rule.trim()).filter(Boolean)
  const urlList = urls.split('\n').map((url) => url.trim()).filter(Boolean)
  const robotsArgs = ua.trim() && ruleList.length > 0 ? { groups: [{ userAgent: ua.trim(), rules: ruleList }], sitemaps: [] } : null
  const robots = useDebouncedCall<RobotsOut>(tab === 'robots' && robotsArgs ? 'robots' : null, robotsArgs, [ua, rules])
  const sitemapArgs = tab === 'sitemap' && urlList.length > 0 ? { urls: urlList, changefreq } : null
  const sitemap = useDebouncedCall<SitemapOut>(sitemapArgs ? 'sitemap' : null, sitemapArgs, [urls, changefreq])

  const output = tab === 'robots' ? robots.result?.text : sitemap.result?.xml
  const copy = () => {
    if (output) void navigator.clipboard.writeText(output)
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <SegmentedControl
        value={tab}
        onValueChange={setTab}
        aria-label={t('ui.robotsTab')}
        options={[
          { value: 'robots', label: t('ui.robotsTab') },
          { value: 'sitemap', label: t('ui.sitemapTab') },
        ]}
      />
      <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
        <Panel icon={<Bot />} title={tab === 'robots' ? t('ui.ua') : t('api.sitemap.summary')} bodyClassName="space-y-2.5 p-3">
          {tab === 'robots' ? (
            <>
              <Input value={ua} onChange={(event) => setUa(event.target.value)} placeholder={t('ui.ua')} aria-label={t('ui.ua')} />
              <textarea
                value={rules}
                onChange={(event) => setRules(event.target.value)}
                aria-label={t('ui.rulesHint')}
                className="h-24 w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-xs text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
              />
              <p className="text-[11px] text-fg-subtle">{t('ui.rulesHint')}</p>
            </>
          ) : (
            <>
              <textarea
                value={urls}
                onChange={(event) => setUrls(event.target.value)}
                aria-label={t('api.sitemap.urls')}
                className="h-32 w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-xs text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Input value={changefreq} onChange={(event) => setChangefreq(event.target.value)} placeholder={t('api.sitemap.changefreq')} aria-label={t('api.sitemap.changefreq')} />
            </>
          )}
        </Panel>
        <Panel
          icon={<Bot />}
          title={tab === 'robots' ? t('ui.robotsTab') : t('ui.sitemapTab')}
          className="min-h-0"
          bodyClassName="min-h-0 overflow-auto space-y-2 p-3"
          extra={(tab === 'robots' ? robots.pending : sitemap.pending) && <Spinner className="size-3.5" />}
        >
          {(tab === 'robots' ? robots.error : sitemap.error) ? (
            <p className="text-xs text-danger">{errorMessage((tab === 'robots' ? robots.error : sitemap.error)!)}</p>
          ) : (
            output && (
              <>
                {tab === 'sitemap' && sitemap.result && sitemap.result.warnings.length > 0 && (
                  <div className="rounded-control bg-warning-soft p-2 text-[11px] text-warning">
                    {sitemap.result.warnings.map((warning) => <p key={warning}>{warning}</p>)}
                  </div>
                )}
                <textarea readOnly value={output} aria-label={t('ui.rules')} className="h-64 w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-[11px] text-fg outline-none" />
                <Button size="sm" variant="secondary" onClick={copy}>
                  <Copy className="size-3.5" />
                  {t('ui.copy')}
                </Button>
              </>
            )
          )}
        </Panel>
      </div>
    </div>
  )
}
