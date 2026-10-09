import { useState } from 'react'
import { Badge, Button, Empty, Input, Panel, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, useLaunchInput, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { ArrowDownAZ, ClipboardPaste, Eraser, Link2, ListTree, Plus, SlidersHorizontal, Trash2 } from 'lucide-react'

interface ParamInfo {
  key: string
  value: string
  raw: string
  tracking: boolean
  duplicate: boolean
}
interface Parsed {
  href: string
  readable: string
  assumedScheme: boolean
  scheme: string
  username: string
  password: string | null
  host: string | null
  hostKind: 'domain' | 'ipv4' | 'ipv6' | null
  hostUnicode: string | null
  port: number | null
  defaultPort: number | null
  origin: string
  path: string
  segments: string[]
  query: string | null
  params: ParamInfo[]
  fragment: string | null
}
interface Param {
  id: number
  key: string
  value: string
}

const EXAMPLE = 'https://shop.example.com:8443/商品/详情?id=42&name=%E5%BC%A0%E4%B8%89&utm_source=newsletter&utm_medium=email&spm=a1.b2#reviews'
let nextId = 0

function Part({ label, value, hint }: { label: string; value: string | null | undefined; hint?: string }) {
  if (!value) return null
  return (
    <div className="group flex items-start gap-3 rounded-control px-3 py-2 hover:bg-hover">
      <span className="w-20 shrink-0 pt-0.5 text-xs font-medium text-fg-muted">{label}</span>
      <div className="min-w-0 flex-1">
        <p className="font-mono text-[12.5px] break-all text-fg" data-selectable>
          {value}
        </p>
        {hint && <p className="text-[11px] text-fg-subtle">{hint}</p>}
      </div>
      <CopyButton text={value} className="-my-1 opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
    </div>
  )
}

function UrlParser() {
  const { t, call, errorMessage } = usePlugin()
  const [input, setInput] = usePluginState('input', EXAMPLE)
  const [spaceAsPlus, setSpaceAsPlus] = usePluginState('spaceAsPlus', false)
  const [params, setParams] = useState<Param[]>([])
  useLaunchInput((text) => setInput(text))

  const parsed = useDebouncedCall<Parsed>(input.trim() ? 'parse' : null, { input }, [input])
  const p = parsed.result
  // 解析结果变化时，用其中的参数重置可编辑列表
  const [seen, setSeen] = useState<Parsed | null>(null)
  if (p !== seen) {
    setSeen(p)
    if (p) setParams(p.params.map((x) => ({ id: nextId++, key: x.key, value: x.value })))
  }
  const edited = p && (params.length !== p.params.length || params.some((x, i) => x.key !== p.params[i].key || x.value !== p.params[i].value))
  const buildArgs = p ? { input, params: params.map(({ key, value }) => ({ key, value })), spaceAsPlus } : null
  const built = useDebouncedCall<{ href: string }>(buildArgs ? 'build' : null, buildArgs, [input, params, spaceAsPlus])
  const info = (key: string, i: number) => (p && p.params[i]?.key === key ? p.params[i] : null)

  const paste = async () => {
    try {
      const text = await host.clipboard.readText()
      if (text) setInput(text.trim())
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const strip = async () => {
    try {
      const out = await call<{ href: string; removed: number }>('strip_tracking', { input })
      setInput(out.href)
      toast.success(t('params.stripped', { count: out.removed }))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const update = (id: number, field: 'key' | 'value', text: string) => setParams((prev) => prev.map((x) => (x.id === id ? { ...x, [field]: text } : x)))
  const trackingCount = p?.params.filter((x) => x.tracking).length ?? 0

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel icon={<Link2 />} title={t('input.title')} className="shrink-0" bodyClassName="space-y-2 p-3">
        <div className="flex gap-2">
          <Input value={input} onChange={(e) => setInput(e.target.value)} placeholder={t('input.placeholder')} className="font-mono" wrapperClassName="min-w-0 flex-1" aria-label={t('input.title')} autoFocus />
          <Button onClick={paste}>
            <ClipboardPaste />
            {t('input.paste')}
          </Button>
        </div>
        {p?.assumedScheme && <p className="text-[11px] text-fg-subtle">{t('input.assumed')}</p>}
        {parsed.error && <p className="text-[12px] text-danger">{errorMessage(parsed.error)}</p>}
      </Panel>

      {!input.trim() ? (
        <Panel className="min-h-0 flex-1">
          <Empty icon={<Link2 />} title={t('input.empty')} description={t('input.emptyHint')} />
        </Panel>
      ) : (
        <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3">
          <Panel icon={<ListTree />} title={t('parts.title')} bodyClassName={cn('overflow-auto p-1.5', parsed.pending && 'opacity-60')}>
            {p && (
              <>
                <Part label={t('parts.href')} value={p.href} />
                {p.readable !== p.href && <Part label={t('parts.readable')} value={p.readable} />}
                <Part label={t('parts.scheme')} value={p.scheme} />
                <Part label={t('parts.username')} value={p.username} />
                <Part label={t('parts.password')} value={p.password} />
                <Part label={t('parts.host')} value={p.host} hint={p.hostKind ? t(`hostKinds.${p.hostKind}`) : undefined} />
                <Part label={t('parts.hostUnicode')} value={p.hostUnicode} />
                <Part
                  label={t('parts.port')}
                  value={String(p.port ?? p.defaultPort ?? '') || null}
                  hint={p.port === null && p.defaultPort ? t('parts.defaultPort') : p.port !== null && p.port === p.defaultPort ? t('parts.redundantPort') : undefined}
                />
                <Part label={t('parts.origin')} value={p.origin !== 'null' ? p.origin : null} />
                <Part label={t('parts.path')} value={p.path} />
                {p.segments.length > 1 && (
                  <div className="flex items-start gap-3 px-3 py-2">
                    <span className="w-20 shrink-0 pt-0.5 text-xs font-medium text-fg-muted">{t('parts.segments')}</span>
                    <div className="flex flex-wrap gap-1">
                      {p.segments.map((segment, i) => (
                        <Badge key={i}>{segment}</Badge>
                      ))}
                    </div>
                  </div>
                )}
                <Part label={t('parts.query')} value={p.query} />
                <Part label={t('parts.fragment')} value={p.fragment} />
              </>
            )}
          </Panel>

          <Panel
            icon={<SlidersHorizontal />}
            title={t('params.title')}
            extra={params.length > 0 && <Badge>{params.length}</Badge>}
            actions={
              <>
                <Tooltip content={t('params.stripHint')}>
                  <Button size="sm" disabled={trackingCount === 0} onClick={strip}>
                    <Eraser />
                    {t('params.strip', { count: trackingCount })}
                  </Button>
                </Tooltip>
                <Button size="icon-sm" variant="ghost" aria-label={t('params.sort')} disabled={params.length < 2} onClick={() => setParams((prev) => [...prev].sort((a, b) => a.key.localeCompare(b.key)))}>
                  <ArrowDownAZ />
                </Button>
                <Button size="icon-sm" variant="ghost" aria-label={t('params.add')} disabled={!p} onClick={() => setParams((prev) => [...prev, { id: nextId++, key: '', value: '' }])}>
                  <Plus />
                </Button>
              </>
            }
            footer={
              p && (
                <div className="flex w-full min-w-0 items-center gap-2">
                  <label className="flex shrink-0 items-center gap-1.5 text-xs text-fg-muted">
                    <Switch size="sm" checked={spaceAsPlus} onCheckedChange={setSpaceAsPlus} aria-label={t('params.plus')} />
                    {t('params.plus')}
                  </label>
                  {edited && built.result && (
                    <>
                      <code className="min-w-0 flex-1 truncate font-mono text-[11px] text-fg" title={built.result.href}>
                        {built.result.href}
                      </code>
                      <CopyButton text={built.result.href} />
                      <Button size="sm" variant="primary" onClick={() => setInput(built.result!.href)}>
                        {t('params.apply')}
                      </Button>
                    </>
                  )}
                </div>
              )
            }
            bodyClassName="overflow-auto p-2"
          >
            {params.length === 0 ? (
              <Empty icon={<SlidersHorizontal />} title={t('params.empty')} description={t('params.emptyHint')} />
            ) : (
              <div className="space-y-1.5">
                {params.map((param, i) => {
                  const original = info(param.key, i)
                  return (
                    <div key={param.id} className="group flex items-center gap-1.5">
                      <Input size="sm" value={param.key} onChange={(e) => update(param.id, 'key', e.target.value)} placeholder={t('params.key')} className="font-mono" wrapperClassName="w-36 shrink-0" aria-label={t('params.key')} />
                      <Input size="sm" value={param.value} onChange={(e) => update(param.id, 'value', e.target.value)} placeholder={t('params.value')} className="font-mono" wrapperClassName="min-w-0 flex-1" aria-label={t('params.value')} />
                      <span className="flex w-24 shrink-0 gap-1">
                        {original?.tracking && <Badge variant="warning">{t('params.tracking')}</Badge>}
                        {original?.duplicate && <Badge variant="info">{t('params.duplicate')}</Badge>}
                      </span>
                      <CopyButton text={param.value} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
                      <Button size="icon-sm" variant="ghost" aria-label={t('params.remove')} onClick={() => setParams((prev) => prev.filter((x) => x.id !== param.id))}>
                        <Trash2 />
                      </Button>
                    </div>
                  )
                })}
              </div>
            )}
          </Panel>
        </div>
      )}
    </div>
  )
}

export default UrlParser
