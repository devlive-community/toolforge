import {
  Binary,
  Braces,
  Calculator,
  Coffee,
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
  { id: 'life', icon: Coffee },
  { id: 'other', icon: LayoutGrid },
]

export const KNOWN_CATEGORIES = new Set(CATEGORIES.map((c) => c.id))

/** 已废弃的分类名映射到现有分类（旧插件包仍然带着这些值） */
const ALIASES: Record<string, string> = { graphics: 'image' }

/** 插件声明的分类 → 宿主分类：别名换成对应分类，其余未知分类兜底到「其他」 */
export function normalizeCategory(category: string): string {
  const id = ALIASES[category] ?? category
  return KNOWN_CATEGORIES.has(id) ? id : 'other'
}
