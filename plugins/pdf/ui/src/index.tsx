import { useEffect, useRef, useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Images as ImagesIcon, LayoutGrid, Scissors } from 'lucide-react'
import { Images } from './Images'
import { Organize } from './Organize'
import { Split } from './Split'

type Tab = 'organize' | 'split' | 'images'

function PdfTools() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('organize')
  const [drops, setDrops] = useState<{ organize: string[]; split: string | null; images: string[] }>({ organize: [], split: null, images: [] })
  const tabRef = useRef(tab)
  useEffect(() => {
    tabRef.current = tab
  }, [tab])

  // 拖入的文件交给当前页签
  useEffect(() => {
    const off = host.onFileDrop({
      drop: (paths) => {
        const current = tabRef.current
        setDrops((prev) => (current === 'split' ? { ...prev, split: paths[0] ?? null } : { ...prev, [current]: paths }))
      },
    })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Tabs<Tab>
        value={tab}
        onValueChange={setTab}
        className="self-start"
        aria-label={t('name')}
        items={[
          { value: 'organize', label: t('tabs.organize'), icon: <LayoutGrid /> },
          { value: 'split', label: t('tabs.split'), icon: <Scissors /> },
          { value: 'images', label: t('tabs.images'), icon: <ImagesIcon /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        <div className="h-full" hidden={tab !== 'organize'}>
          <Organize dropped={drops.organize} />
        </div>
        <div className="h-full" hidden={tab !== 'split'}>
          <Split dropped={drops.split} />
        </div>
        <div className="h-full" hidden={tab !== 'images'}>
          <Images dropped={drops.images} />
        </div>
      </div>
    </div>
  )
}

export default PdfTools
