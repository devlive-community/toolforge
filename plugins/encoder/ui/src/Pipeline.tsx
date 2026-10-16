import { useState } from 'react'
import { Button, DropdownMenu, Input, Select, SegmentedControl, Tooltip } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowRight, ChevronDown, Plus, Save, Trash2, X } from 'lucide-react'
import { GROUPS, type Codec, type Direction, type Step } from './types'

export interface Preset {
  name: string
  steps: Step[]
}

/** 内置预设：接口传输中常见的套娃编码 */
const BUILTIN: { key: string; steps: Step[] }[] = [
  { key: 'base64Gunzip', steps: [{ codec: 'base64', direction: 'decode' }, { codec: 'gzip', direction: 'decode' }] },
  { key: 'gzipBase64', steps: [{ codec: 'gzip', direction: 'encode' }, { codec: 'base64', direction: 'encode' }] },
  { key: 'base64Inflate', steps: [{ codec: 'base64', direction: 'decode' }, { codec: 'zlib', direction: 'decode' }] },
  { key: 'urlBase64', steps: [{ codec: 'urlComponent', direction: 'decode' }, { codec: 'base64', direction: 'decode' }] },
]

const ALL_CODECS: Codec[] = GROUPS.flatMap((group) => group.codecs)

interface Props {
  steps: Step[]
  onChange: (steps: Step[]) => void
  presets: Preset[]
  onPresetsChange: (presets: Preset[]) => void
  failedStep: number | null
}

export function Pipeline({ steps, onChange, presets, onPresetsChange, failedStep }: Props) {
  const { t } = usePlugin()
  const [naming, setNaming] = useState<string | null>(null)
  const update = (index: number, patch: Partial<Step>) => onChange(steps.map((step, i) => (i === index ? { ...step, ...patch } : step)))

  const save = () => {
    const name = naming?.trim()
    if (!name) return
    onPresetsChange([...presets.filter((p) => p.name !== name), { name, steps }])
    setNaming(null)
  }

  return (
    <div className="flex w-full flex-col gap-2">
      <div className="flex flex-wrap items-center gap-1.5">
        {steps.map((step, index) => (
          <div key={index} className="flex items-center gap-1.5">
            {index > 0 && <ArrowRight className="size-3.5 text-fg-subtle" />}
            <div className={`flex items-center gap-1 rounded-control border px-1.5 py-1 ${failedStep === index + 1 ? 'border-danger' : 'border-border'}`}>
              <span className="px-1 text-[11px] text-fg-subtle tabular-nums">{index + 1}</span>
              <Select<Codec>
                size="sm"
                value={step.codec}
                onValueChange={(codec) => update(index, { codec })}
                aria-label={t('pipeline.codec', { index: index + 1 })}
                options={ALL_CODECS.map((codec) => ({ value: codec, label: t(`codecs.${codec}.name`) }))}
              />
              <SegmentedControl<Direction>
                size="sm"
                value={step.direction}
                onValueChange={(direction) => update(index, { direction })}
                aria-label={t('direction.label')}
                options={[
                  { value: 'encode', label: t(`pipeline.${step.codec === 'gzip' || step.codec === 'deflate' || step.codec === 'zlib' || step.codec === 'zstd' ? 'compress' : 'encode'}`) },
                  { value: 'decode', label: t(`pipeline.${step.codec === 'gzip' || step.codec === 'deflate' || step.codec === 'zlib' || step.codec === 'zstd' ? 'decompress' : 'decode'}`) },
                ]}
              />
              <Tooltip content={t('pipeline.remove')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('pipeline.remove')} disabled={steps.length === 1} onClick={() => onChange(steps.filter((_, i) => i !== index))}>
                  <X />
                </Button>
              </Tooltip>
            </div>
          </div>
        ))}
        <Button size="sm" onClick={() => onChange([...steps, { codec: 'base64', direction: 'encode' }])} disabled={steps.length >= 16}>
          <Plus />
          {t('pipeline.add')}
        </Button>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <DropdownMenu
          placement="bottom-start"
          trigger={
            <Button size="sm">
              {t('pipeline.presets')}
              <ChevronDown />
            </Button>
          }
          items={[
            ...BUILTIN.map((preset) => ({ key: preset.key, label: t(`pipeline.builtin.${preset.key}`), onSelect: () => onChange(preset.steps) })),
            ...presets.map((preset) => ({ key: `user:${preset.name}`, label: preset.name, onSelect: () => onChange(preset.steps) })),
          ]}
        />
        {naming === null ? (
          <Button size="sm" variant="ghost" onClick={() => setNaming('')}>
            <Save />
            {t('pipeline.save')}
          </Button>
        ) : (
          <span className="flex items-center gap-1.5">
            <Input size="sm" value={naming} onChange={(event) => setNaming(event.target.value)} onKeyDown={(event) => event.key === 'Enter' && save()} placeholder={t('pipeline.name')} aria-label={t('pipeline.name')} autoFocus />
            <Button size="sm" variant="primary" onClick={save} disabled={!naming.trim()}>
              {t('pipeline.confirm')}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setNaming(null)}>
              {t('common:cancel')}
            </Button>
          </span>
        )}
        {presets.length > 0 && (
          <DropdownMenu
            placement="bottom-start"
            trigger={
              <Button size="sm" variant="ghost">
                <Trash2 />
                {t('pipeline.delete')}
              </Button>
            }
            items={presets.map((preset) => ({ key: preset.name, label: preset.name, onSelect: () => onPresetsChange(presets.filter((p) => p.name !== preset.name)) }))}
          />
        )}
      </div>
    </div>
  )
}
