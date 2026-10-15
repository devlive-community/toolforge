import { release, type ReleaseData } from '@/lib/catalog'

export type DownloadLabel = 'appleSilicon' | 'intel' | 'x64' | 'arm64' | 'appimage' | 'deb'
export type DownloadPlatform = 'macos' | 'windows' | 'linux'
export type DownloadGroup = { name: DownloadPlatform; items: { labelKey: DownloadLabel; url: string }[] }

export function getDownloadGroups(data: ReleaseData = release): DownloadGroup[] {
  if (data.fallback) return []
  const pair = (labelKey: DownloadLabel, url?: string) => url ? { labelKey, url } : null
  const groups: DownloadGroup[] = [
    { name: 'macos', items: [pair('appleSilicon', data.assets.macosApple), pair('intel', data.assets.macosIntel)].filter((item) => item !== null) },
    { name: 'windows', items: [pair('x64', data.assets.windowsX64), pair('arm64', data.assets.windowsArm)].filter((item) => item !== null) },
    { name: 'linux', items: [pair('appimage', data.assets.linuxAppImage), pair('deb', data.assets.linuxDeb)].filter((item) => item !== null) },
  ]
  return groups.filter((group) => group.items.length > 0)
}
