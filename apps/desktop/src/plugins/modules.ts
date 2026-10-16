import { lazy, type ComponentType, type LazyExoticComponent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { Manifest, PluginModule } from '@toolforge/plugin-ui-sdk'
import { addPluginResources } from '../i18n'
import { boot } from '../lib/boot'

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

/** 在线安装插件的图标（来自插件包） */
const installedIcons = new Map<string, string>()
/** 已注册资源的在线插件及其版本，版本变化（更新）时重新注册 */
const installedVersions = new Map<string, string>()

/** 在线插件的文件地址：Windows 的 WebView2 使用 http://<scheme>.localhost */
function installedUrl(id: string, file: string) {
  const base = boot.os === 'windows' ? 'http://tfplugin.localhost' : 'tfplugin://localhost'
  return `${base}/${id}/${file}`
}

export const isBuiltin = (id: string) => uiById.has(id)

interface InstalledAssets {
  locales: Record<string, object>
  icon: string | null
  css: boolean
}

/** 注册在线插件的语言包、图标与样式（启动时与安装、更新后调用） */
export async function registerInstalled(plugins: Manifest[]) {
  await Promise.all(
    plugins
      .filter((p) => !isBuiltin(p.id) && installedVersions.get(p.id) !== p.version)
      .map(async (p) => {
        const assets = await invoke<InstalledAssets>('plugin_assets', { pluginId: p.id })
        addPluginResources(p.id, assets.locales)
        if (assets.icon) installedIcons.set(p.id, assets.icon)
        document.querySelector(`link[data-plugin="${p.id}"]`)?.remove()
        if (assets.css) {
          const link = document.createElement('link')
          link.rel = 'stylesheet'
          link.dataset.plugin = p.id
          link.href = `${installedUrl(p.id, 'ui/index.css')}?v=${encodeURIComponent(p.version)}`
          document.head.append(link)
        }
        // 更新后重新加载界面模块（带版本参数避免命中旧模块）
        components.delete(p.id)
        installedVersions.set(p.id, p.version)
      }),
  )
}

export function pluginIcon(id: string): string | undefined {
  return iconById.get(id) ?? installedIcons.get(id)
}

export function pluginComponent(id: string) {
  const version = installedVersions.get(id)
  const loader: (() => Promise<PluginModule>) | undefined =
    uiById.get(id) ??
    (version ? () => import(/* @vite-ignore */ `${installedUrl(id, 'ui/index.js')}?v=${encodeURIComponent(version)}`) : undefined)
  if (!loader) return null
  let component = components.get(id)
  if (!component) {
    component = lazy(loader)
    components.set(id, component)
  }
  return component
}
