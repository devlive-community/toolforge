import { useState } from 'react'
import { Button, Empty, Input, NumberInput, Panel, Select, Spinner, Switch } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Fingerprint, RefreshCw, SlidersHorizontal } from 'lucide-react'
import { Field, ToggleRow } from './Field'
import { Inspector } from './Inspector'
import { KINDS, NAMESPACES, isNamed, isUuid, type GenerateResult, type Kind, type Namespace } from './types'

export function UuidTool() {
  const { t, errorMessage } = usePlugin()
  const [kind, setKind] = useState<Kind>('v4')
  const [count, setCount] = useState(10)
  const [uppercase, setUppercase] = useState(false)
  const [hyphens, setHyphens] = useState(true)
  const [braces, setBraces] = useState(false)
  const [namespace, setNamespace] = useState<Namespace>('dns')
  const [customNamespace, setCustomNamespace] = useState('')
  const [name, setName] = useState('toolforge.dev')
  const [nanoidSize, setNanoidSize] = useState(21)
  const [nanoidAlphabet, setNanoidAlphabet] = useState('')
  const [nonce, setNonce] = useState(0)

  const args = { kind, count, uppercase, hyphens, braces, namespace, customNamespace, name, nanoidSize, nanoidAlphabet }
  const { result, error, pending } = useDebouncedCall<GenerateResult>('generate', args, [
    kind, count, uppercase, hyphens, braces, namespace, customNamespace, name, nanoidSize, nanoidAlphabet, nonce,
  ])
  const labelWidth = String(result?.ids.length ?? 0).length

  return (
    <div className="grid h-full grid-cols-[minmax(300px,360px)_minmax(0,1fr)] gap-3">
      <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
        <Panel icon={<SlidersHorizontal />} title={t('settings.title')} bodyClassName="space-y-4 p-4">
          <Field label={t('settings.kind')}>
            <Select<Kind>
              value={kind}
              onValueChange={setKind}
              className="w-full"
              aria-label={t('settings.kind')}
              options={KINDS.map((value) => ({ value, label: t(`kinds.${value}`) }))}
            />
          </Field>

          {!isNamed(kind) && (
            <Field label={t('settings.count')}>
              <NumberInput
                value={count}
                onValueChange={setCount}
                min={1}
                max={1000}
                className="w-full"
                aria-label={t('settings.count')}
                decrementLabel={t('settings.decrease')}
                incrementLabel={t('settings.increase')}
              />
            </Field>
          )}

          {isNamed(kind) && (
            <>
              <Field label={t('settings.namespace')}>
                <Select<Namespace>
                  value={namespace}
                  onValueChange={setNamespace}
                  className="w-full"
                  aria-label={t('settings.namespace')}
                  options={NAMESPACES.map((value) => ({ value, label: t(`namespaces.${value}`) }))}
                />
              </Field>
              {namespace === 'custom' && (
                <Input
                  value={customNamespace}
                  onChange={(event) => setCustomNamespace(event.target.value)}
                  placeholder="6ba7b810-9dad-11d1-80b4-00c04fd430c8"
                  className="font-mono"
                  aria-label={t('namespaces.custom')}
                />
              )}
              <Field label={t('settings.name')} hint={t('settings.nameHint')}>
                <Input value={name} onChange={(event) => setName(event.target.value)} aria-label={t('settings.name')} />
              </Field>
            </>
          )}

          {kind === 'nanoid' && (
            <>
              <Field label={t('settings.size')}>
                <NumberInput value={nanoidSize} onValueChange={setNanoidSize} min={1} max={256} className="w-full" aria-label={t('settings.size')} />
              </Field>
              <Field label={t('settings.alphabet')} hint={t('settings.alphabetHint')}>
                <Input value={nanoidAlphabet} onChange={(event) => setNanoidAlphabet(event.target.value)} className="font-mono" aria-label={t('settings.alphabet')} />
              </Field>
            </>
          )}

          {(isUuid(kind) || kind === 'ulid') && (
            <div className="rounded-control border border-border px-3 py-1.5">
              <ToggleRow label={t('settings.uppercase')}>
                <Switch size="sm" checked={uppercase} onCheckedChange={setUppercase} aria-label={t('settings.uppercase')} />
              </ToggleRow>
              {isUuid(kind) && (
                <>
                  <ToggleRow label={t('settings.hyphens')}>
                    <Switch size="sm" checked={hyphens} onCheckedChange={setHyphens} aria-label={t('settings.hyphens')} />
                  </ToggleRow>
                  <ToggleRow label={t('settings.braces')}>
                    <Switch size="sm" checked={braces} onCheckedChange={setBraces} aria-label={t('settings.braces')} />
                  </ToggleRow>
                </>
              )}
            </div>
          )}

          <Button variant="primary" block onClick={() => setNonce((n) => n + 1)} disabled={isNamed(kind)}>
            <RefreshCw />
            {t('settings.regenerate')}
          </Button>
        </Panel>
        <Inspector />
      </div>

      <Panel
        icon={<Fingerprint />}
        title={t('result.title')}
        extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
        actions={result && <CopyButton text={result.ids.join('\n')} label={t('result.copyAll')} variant="outline" />}
        footer={
          result && (
            <>
              <span>{t('result.count', { count: result.ids.length })}</span>
              {result.deterministic && <span>{t('result.deterministic')}</span>}
              <span className="ml-auto">{t('result.elapsed', { ms: result.elapsedMs })}</span>
            </>
          )
        }
        bodyClassName="overflow-auto p-2"
      >
        {error ? (
          <p className="p-4 text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result ? (
          <Empty icon={<Fingerprint />} title={t('result.empty')} />
        ) : (
          <ol className="font-mono text-[13px]" data-selectable>
            {result.ids.map((id, index) => (
              <li key={`${index}-${id}`} className="group flex items-center gap-3 rounded-control px-3 py-1.5 hover:bg-hover">
                <span className="shrink-0 text-right text-xs text-fg-subtle tabular-nums" style={{ width: `${labelWidth}ch` }}>
                  {index + 1}
                </span>
                <code className="min-w-0 flex-1 break-all text-fg">{id}</code>
                <CopyButton text={id} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
              </li>
            ))}
          </ol>
        )}
      </Panel>
    </div>
  )
}
