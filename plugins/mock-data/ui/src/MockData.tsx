import { Badge, Button, CodeEditor, DropdownMenu, Input, NumberInput, Panel, SegmentedControl, Select, Spinner, Tooltip, toast, type CodeLanguage } from '@toolforge/ui'
import { CopyButton, formatBytes, host, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Dices, Download, LayoutTemplate, ListPlus, Rows3, TableProperties } from 'lucide-react'
import { FieldRow } from './FieldRow'
import { PRESETS, newKey } from './presets'
import type { Config, Dialect, Field, Format, Locale, Preview } from './types'

const EXTENSIONS: Record<Format, string> = { json: 'json', jsonl: 'jsonl', csv: 'csv', sql: 'sql' }
const EDITOR_LANGUAGE: Record<Format, CodeLanguage> = { json: 'json', jsonl: 'text', csv: 'text', sql: 'sql' }
const randomSeed = () => Math.floor(Math.random() * 2 ** 31)

const DEFAULT: Config = {
  locale: 'zh-cn',
  rows: 100,
  seed: 20260925,
  format: 'json',
  dialect: 'mysql',
  table: 'users',
  fields: PRESETS.users(),
}

/** 发给后端的参数：去掉界面专用的 key */
function request(config: Config) {
  return {
    locale: config.locale,
    rows: config.rows,
    seed: config.seed,
    format: config.format,
    dialect: config.dialect,
    table: config.table,
    fields: config.fields.map(({ key: _key, ...field }) => field),
  }
}

export function MockData() {
  const { t, call, errorMessage } = usePlugin()
  const [config, setConfig] = usePluginState<Config>('config', DEFAULT)
  const update = (patch: Partial<Config>) => setConfig({ ...config, ...patch })
  const args = request(config)
  const { result, error, pending } = useDebouncedCall<Preview>('generate', args, [JSON.stringify(args)])

  const setField = (index: number, field: Field) => update({ fields: config.fields.map((f, i) => (i === index ? field : f)) })
  const move = (index: number, delta: number) => {
    const fields = [...config.fields]
    const [item] = fields.splice(index, 1)
    fields.splice(index + delta, 0, item)
    update({ fields })
  }
  const addField = () =>
    update({ fields: [...config.fields, { key: newKey(), name: `field_${config.fields.length + 1}`, kind: 'word', nullPercent: 0 }] })

  const save = async () => {
    try {
      const ext = EXTENSIONS[config.format]
      const path = await host.dialog.saveFile(`${config.table || 'mock_data'}.${ext}`, [{ name: ext.toUpperCase(), extensions: [ext] }])
      if (!path) return
      const saved = await call<{ rows: number; bytes: number }>('save', { ...args, path })
      toast.success(t('output.saved', { rows: saved.rows, size: formatBytes(saved.bytes) }))
    } catch (err) {
      toast.error(errorMessage(err))
    }
  }

  const partial = result && result.previewRows < result.rows

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
      <section className="flex flex-wrap items-center gap-3 rounded-card border border-border bg-surface px-3 py-2 shadow-card">
        <SegmentedControl<Locale>
          size="sm"
          value={config.locale}
          onValueChange={(locale) => update({ locale })}
          aria-label={t('settings.locale')}
          options={[
            { value: 'zh-cn', label: t('settings.chinese') },
            { value: 'en-us', label: t('settings.english') },
          ]}
        />
        <label className="flex items-center gap-1.5 text-xs text-fg-muted">
          {t('settings.rows')}
          <NumberInput size="sm" value={config.rows} onValueChange={(rows) => update({ rows })} min={1} max={100000} step={100} className="w-32" aria-label={t('settings.rows')} />
        </label>
        <SegmentedControl<Format>
          size="sm"
          value={config.format}
          onValueChange={(format) => update({ format })}
          aria-label={t('settings.format')}
          options={[
            { value: 'json', label: 'JSON' },
            { value: 'jsonl', label: 'JSONL' },
            { value: 'csv', label: 'CSV' },
            { value: 'sql', label: 'SQL' },
          ]}
        />
        {config.format === 'sql' && (
          <>
            <Select<Dialect>
              size="sm"
              value={config.dialect}
              onValueChange={(dialect) => update({ dialect })}
              className="w-32"
              aria-label={t('settings.dialect')}
              options={[
                { value: 'mysql', label: 'MySQL' },
                { value: 'postgres', label: 'PostgreSQL' },
                { value: 'sqlite', label: 'SQLite' },
              ]}
            />
            <Input size="sm" value={config.table} onChange={(event) => update({ table: event.target.value })} placeholder={t('settings.table')} className="font-mono" wrapperClassName="w-36" aria-label={t('settings.table')} spellCheck={false} />
          </>
        )}
        <div className="ml-auto flex items-center gap-1.5">
          <Tooltip content={t('settings.seedHint')}>
            <label className="flex items-center gap-1.5 text-xs text-fg-muted">
              {t('settings.seed')}
              <Input
                size="sm"
                value={String(config.seed)}
                onChange={(event) => update({ seed: Number(event.target.value.replace(/\D/g, '').slice(0, 15)) || 0 })}
                className="font-mono"
                wrapperClassName="w-32"
                aria-label={t('settings.seed')}
              />
            </label>
          </Tooltip>
          <Tooltip content={t('settings.shuffle')}>
            <Button size="icon-sm" aria-label={t('settings.shuffle')} onClick={() => update({ seed: randomSeed() })}>
              <Dices />
            </Button>
          </Tooltip>
        </div>
      </section>

      <div className="grid min-h-0 grid-cols-[minmax(440px,0.9fr)_minmax(0,1.1fr)] gap-3">
        <Panel
          icon={<TableProperties />}
          title={t('fields.title')}
          extra={<Badge>{config.fields.length}</Badge>}
          actions={
            <>
              <DropdownMenu
                trigger={
                  <Button size="sm">
                    <LayoutTemplate />
                    {t('presets.title')}
                  </Button>
                }
                items={Object.keys(PRESETS).map((key) => ({
                  key,
                  label: t(`presets.${key}`),
                  onSelect: () => update({ fields: PRESETS[key](), table: key }),
                }))}
              />
              <Button size="sm" onClick={addField} disabled={config.fields.length >= 50}>
                <ListPlus />
                {t('fields.add')}
              </Button>
            </>
          }
          bodyClassName="space-y-2 overflow-auto p-3"
        >
          {config.fields.map((field, index) => (
            <FieldRow
              key={field.key}
              field={field}
              first={index === 0}
              last={index === config.fields.length - 1}
              onChange={(next) => setField(index, next)}
              onMove={(delta) => move(index, delta)}
              onRemove={() => update({ fields: config.fields.filter((_, i) => i !== index) })}
            />
          ))}
        </Panel>

        <Panel
          icon={<Rows3 />}
          title={t('output.title')}
          extra={pending ? <Spinner className="size-3.5 text-fg-subtle" /> : result && <Badge>{partial ? t('output.partial', { shown: result.previewRows, total: result.rows }) : t('output.rows', { count: result.rows })}</Badge>}
          actions={
            <>
              <CopyButton text={result?.output ?? ''} label={partial ? t('output.copyPreview') : t('common:copy')} variant="outline" disabled={!result || !!error} />
              <Button size="sm" variant="primary" onClick={save} disabled={!!error}>
                <Download />
                {t('output.save')}
              </Button>
            </>
          }
          footer={error ? <span className="truncate text-danger">{errorMessage(error)}</span> : partial ? <span>{t('output.partialHint')}</span> : null}
        >
          <CodeEditor value={error ? '' : (result?.output ?? '')} readOnly language={EDITOR_LANGUAGE[config.format]} aria-label={t('output.title')} />
        </Panel>
      </div>
    </div>
  )
}
