import { useEffect, useRef, useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FolderOpen, PackagePlus } from 'lucide-react'
import { Browse } from './Browse'
import { Create } from './Create'

type Tab = 'open' | 'create'

function Archive() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('open')
  const [openDrop, setOpenDrop] = useState<string | null>(null)
  const [createDrop, setCreateDrop] = useState<string[]>([])
  const tabRef = useRef(tab)
  useEffect(() => {
    tabRef.current = tab
  }, [tab])

  // 拖入的文件：在“打开”页打开第一个压缩包，在“创建”页加入待压缩列表
  useEffect(() => {
    const off = host.onFileDrop({
      drop: (paths) => {
        if (tabRef.current === 'open') setOpenDrop(paths[0] ?? null)
        else setCreateDrop(paths)
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
          { value: 'open', label: t('tabs.open'), icon: <FolderOpen /> },
          { value: 'create', label: t('tabs.create'), icon: <PackagePlus /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        <div className="h-full" hidden={tab !== 'open'}>
          <Browse dropped={openDrop} />
        </div>
        <div className="h-full" hidden={tab !== 'create'}>
          <Create dropped={createDrop} />
        </div>
      </div>
    </div>
  )
}

export default Archive
