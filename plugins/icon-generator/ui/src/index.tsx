import { useEffect, useRef, useState } from 'react'
import { Tabs, toast } from '@toolforge/ui'
import { host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { LayoutGrid, LayoutTemplate } from 'lucide-react'
import { IconSets } from './IconSets'
import { EXTENSIONS, type Info } from './source'
import { Templates } from './Templates'

type Tab = 'sets' | 'templates'

function IconGenerator() {
  const { t, call, errorMessage } = usePlugin()
  const [tab, setTab] = usePluginState<Tab>('tab', 'sets')
  const [info, setInfo] = useState<Info | null>(null)
  const [hovering, setHovering] = useState(false)

  const load = async (path: string) => {
    try {
      setInfo(await call<Info>('inspect', { path }))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && void loadRef.current(paths[0]), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('source.filter'), extensions: EXTENSIONS }])
      if (path) await load(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Tabs<Tab>
        value={tab}
        onValueChange={setTab}
        className="self-start"
        aria-label={t('name')}
        items={[
          { value: 'sets', label: t('tabs.sets'), icon: <LayoutGrid /> },
          { value: 'templates', label: t('tabs.templates'), icon: <LayoutTemplate /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        <div className="h-full" hidden={tab !== 'sets'}>
          <IconSets key={info?.path ?? ''} info={info} hovering={hovering} onPick={pick} onClear={() => setInfo(null)} />
        </div>
        <div className="h-full" hidden={tab !== 'templates'}>
          <Templates key={info?.path ?? ''} info={info} hovering={hovering} onPick={pick} />
        </div>
      </div>
    </div>
  )
}

export default IconGenerator
