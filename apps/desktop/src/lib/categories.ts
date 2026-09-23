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
