import {
  Binary,
  Braces,
  Calculator,
  FileText,
  Globe,
  Image,
  LayoutGrid,
  Monitor,
  RefreshCcw,
  type LucideIcon,
} from 'lucide-react'

/** 宿主定义的工具分类（插件在 manifest.category 中声明归属） */
export const CATEGORIES: { id: string; icon: LucideIcon }[] = [
  { id: 'dev', icon: Braces },
  { id: 'text', icon: FileText },
  { id: 'encode', icon: Binary },
  { id: 'convert', icon: RefreshCcw },
  { id: 'image', icon: Image },
  { id: 'network', icon: Globe },
  { id: 'system', icon: Monitor },
  { id: 'calc', icon: Calculator },
  { id: 'other', icon: LayoutGrid },
]

export const KNOWN_CATEGORIES = new Set(CATEGORIES.map((c) => c.id))

/** 旧 manifest 的分类（life/graphics 等）已废弃：未知分类统一兜底到「其他」 */
export function normalizeCategory(category: string): string {
  return KNOWN_CATEGORIES.has(category) ? category : 'other'
}
