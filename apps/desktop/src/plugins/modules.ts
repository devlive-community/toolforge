import { lazy, type ComponentType, type LazyExoticComponent } from 'react'
import type { Manifest, PluginModule } from '@toolforge/plugin-ui-sdk'
import { addPluginResources } from '../i18n'

/**
 * 内置插件的前端资源（UI、图标、语言包）按插件目录自动收集，宿主不写死任何工具。
 * 插件列表本身以 Rust 注册表（plugin_list）为准。
 */
const manifests = import.meta.glob<Manifest>('../../../../plugins/*/manifest.json', {
  eager: true,
  import: 'default',
})
const uis = import.meta.glob<PluginModule>('../../../../plugins/*/ui/src/index.tsx')
const icons = import.meta.glob<string>('../../../../plugins/*/icon.svg', {
  eager: true,
  query: '?raw',
  import: 'default',
})
const locales = import.meta.glob<object>('../../../../plugins/*/locales/*.json', {
  eager: true,
  import: 'default',
})

const dirOf = (path: string) => path.split('/plugins/')[1].split('/')[0]
const idByDir = new Map(Object.entries(manifests).map(([path, m]) => [dirOf(path), m.id]))

const byId = <T,>(record: Record<string, T>) => {
  const map = new Map<string, T>()
  for (const [path, value] of Object.entries(record)) {
    const id = idByDir.get(dirOf(path))
    if (id) map.set(id, value)
  }
  return map
}

const uiById = byId(uis)
const iconById = byId(icons)
const components = new Map<string, LazyExoticComponent<ComponentType>>()

/** 注册所有内置插件的语言包（命名空间 = 插件 id） */
export function registerPluginLocales() {
  const bundles = new Map<string, Record<string, object>>()
  for (const [path, resources] of Object.entries(locales)) {
    const id = idByDir.get(dirOf(path))
    if (!id) continue
    const locale = path.split('/').pop()!.replace('.json', '')
    bundles.set(id, { ...bundles.get(id), [locale]: resources })
  }
  bundles.forEach((resources, id) => addPluginResources(id, resources))
}

export function pluginIcon(id: string): string | undefined {
  return iconById.get(id)
}

export function pluginComponent(id: string) {
  const loader = uiById.get(id)
  if (!loader) return null
  let component = components.get(id)
  if (!component) {
    component = lazy(loader)
    components.set(id, component)
  }
  return component
}
