import { useEffect, useState } from 'react'
import { Badge, Button, Empty, NumberInput, Panel, SegmentedControl, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowDown, ArrowUp, ImagePlus, Images as ImagesIcon, Save, X } from 'lucide-react'
import { fileName } from './types'

type Size = 'fit' | 'a4' | 'letter'
const EXTENSIONS = ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp', 'tif', 'tiff']

export function Images({ dropped }: { dropped: string[] }) {
  const { t, errorMessage } = usePlugin()
  const [paths, setPaths] = useState<string[]>([])
  const [size, setSize] = useState<Size>('a4')
  const [margin, setMargin] = useState(24)
  const task = useTask<{ output: string; pages: number; size: number }>()

  const add = (list: string[]) =>
    setPaths((prev) => [...prev, ...list.filter((p) => EXTENSIONS.includes(p.split('.').pop()?.toLowerCase() ?? '') && !prev.includes(p))])
  const [seenDrop, setSeenDrop] = useState(dropped)
  if (dropped !== seenDrop) {
    setSeenDrop(dropped)
    add(dropped)
  }

  useEffect(() => {
    if (task.status === 'succeeded' && task.result) toast.success(t('organize.saved', { pages: task.result.pages, size: formatBytes(task.result.size) }))
    if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [task.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const pick = async () => {
    try {
      add(await host.dialog.openFiles([{ name: t('images.filter'), extensions: EXTENSIONS }]))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const move = (index: number, delta: number) =>
    setPaths((prev) => {
      const next = [...prev]
      const [item] = next.splice(index, 1)
      next.splice(index + delta, 0, item)
      return next
    })
  const create = async () => {
    try {
      const output = await host.dialog.saveFile('images.pdf', [{ name: 'PDF', extensions: ['pdf'] }])
      if (output) task.start('images', { paths, output, pageSize: size, margin })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_300px] gap-3">
      <Panel
        icon={<ImagesIcon />}
        title={t('images.title')}
        extra={paths.length > 0 && <Badge>{paths.length}</Badge>}
        actions={
          <Button size="sm" onClick={pick}>
            <ImagePlus />
            {t('images.add')}
          </Button>
        }
        bodyClassName="overflow-auto p-2"
      >
        {paths.length === 0 ? (
          <Empty icon={<ImagesIcon />} title={t('images.empty')} description={t('images.emptyHint')} />
        ) : (
          <ul className="space-y-0.5">
            {paths.map((path, index) => (
              <li key={path} className="group flex items-center gap-2 rounded-control px-2.5 py-1.5 hover:bg-hover">
                <span className="w-6 text-right text-xs text-fg-subtle tabular-nums">{index + 1}</span>
                <span className="min-w-0 flex-1 truncate text-[13px] text-fg" title={path}>{fileName(path)}</span>
                <Button size="icon-sm" variant="ghost" aria-label={t('images.up')} disabled={index === 0} onClick={() => move(index, -1)}>
                  <ArrowUp />
                </Button>
                <Button size="icon-sm" variant="ghost" aria-label={t('images.down')} disabled={index === paths.length - 1} onClick={() => move(index, 1)}>
                  <ArrowDown />
                </Button>
                <Button size="icon-sm" variant="ghost" aria-label={t('images.remove')} onClick={() => setPaths(paths.filter((p) => p !== path))}>
                  <X />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Panel>
      <Panel title={t('images.options')} bodyClassName="space-y-4 p-3">
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('images.pageSize')}</p>
          <SegmentedControl<Size>
            size="sm"
            value={size}
            onValueChange={setSize}
            aria-label={t('images.pageSize')}
            options={(['fit', 'a4', 'letter'] as Size[]).map((value) => ({ value, label: t(`images.sizes.${value}`) }))}
          />
          <p className="text-[11px] text-fg-subtle">{t(`images.sizeHint.${size}`)}</p>
        </div>
        {size !== 'fit' && (
          <label className="flex items-center justify-between gap-2 text-[13px] text-fg">
            {t('images.margin')}
            <NumberInput value={margin} onValueChange={setMargin} min={0} max={144} step={6} className="w-28" aria-label={t('images.margin')} />
          </label>
        )}
        <p className="text-[11px] leading-snug text-fg-subtle">{t('images.jpegHint')}</p>
        <Button block variant="primary" loading={task.running} disabled={paths.length === 0} onClick={create}>
          <Save />
          {t('images.create')}
        </Button>
        {task.status === 'succeeded' && task.result && (
          <Button block size="sm" variant="ghost" onClick={() => host.revealPath(task.result!.output).catch(() => {})}>
            {t('organize.show')}
          </Button>
        )}
      </Panel>
    </div>
  )
}
