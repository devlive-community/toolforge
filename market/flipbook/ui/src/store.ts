import { loadPluginState, savePluginState } from '@toolforge/plugin-ui-sdk'
import type { Project, WorkMeta } from './types'

/**
 * 存储布局（都在 ToolForge 现有 SQLite 的插件状态里，不新建数据库）：
 * - `works`：作品列表摘要（小）
 * - `work:<作品>`：作品结构与帧顺序（小）
 * - `frame:<作品>:<帧>`：单帧内容（PNG base64），每帧一个键，空白帧不写
 * 帧内容不可变，所以同一个键写入后不会再被改写，只会在不再被引用时删除。
 */
const WORKS = 'works'
const workKey = (id: string) => `work:${id}`
const frameKey = (work: string, frame: string) => `frame:${work}:${frame}`

/** 作品数量上限 */
export const MAX_WORKS = 20

export async function loadWorks(plugin: string): Promise<WorkMeta[]> {
  const works = await loadPluginState<WorkMeta[]>(plugin, WORKS)
  return Array.isArray(works) ? works : []
}

export function saveWorks(plugin: string, works: WorkMeta[]) {
  return savePluginState(plugin, WORKS, works)
}

export function loadProject(plugin: string, id: string) {
  return loadPluginState<Project>(plugin, workKey(id))
}

export function saveProject(plugin: string, project: Project) {
  return savePluginState(plugin, workKey(project.id), project)
}

/** 读取一组帧内容；不存在的键是空白帧 */
export async function loadFrames(plugin: string, work: string, ids: string[]): Promise<Record<string, string>> {
  const entries = await Promise.all(
    ids.map(async (id) => [id, (await loadPluginState<string>(plugin, frameKey(work, id))) ?? ''] as const),
  )
  return Object.fromEntries(entries)
}

/** 写入一帧；空白帧不占存储 */
export async function saveFrame(plugin: string, work: string, id: string, content: string) {
  if (content) await savePluginState(plugin, frameKey(work, id), content)
}

export async function deleteFrames(plugin: string, work: string, ids: string[]) {
  await Promise.all(ids.map((id) => savePluginState(plugin, frameKey(work, id), null)))
}

/** 删除整个作品：先删帧，再删作品结构 */
export async function deleteWork(plugin: string, project: Project) {
  await deleteFrames(plugin, project.id, project.pool)
  await savePluginState(plugin, workKey(project.id), null)
}

/** 作品摘要（列表里显示） */
export function metaOf(project: Project): WorkMeta {
  return {
    id: project.id,
    name: project.name,
    width: project.width,
    height: project.height,
    frames: project.frames.length,
    updatedAt: new Date().toISOString(),
  }
}

/** 帧内容 → 可显示的地址；空白帧返回 null */
export const frameSrc = (content: string | undefined) => (content ? `data:image/png;base64,${content}` : null)
