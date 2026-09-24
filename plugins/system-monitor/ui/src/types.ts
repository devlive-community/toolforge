export interface Snapshot {
  os: {
    name: string | null
    version: string | null
    longVersion: string | null
    kernel: string | null
    hostname: string | null
    arch: string
    uptime: number
    bootTime: number
  }
  cpu: {
    brand: string
    vendor: string
    physicalCores: number | null
    logicalCores: number
    frequency: number
    usage: number
    cores: number[]
    load: [number, number, number]
  }
  memory: { total: number; used: number; available: number; swapTotal: number; swapUsed: number }
  disks: { name: string; mount: string; fileSystem: string; kind: string; total: number; available: number; removable: boolean }[]
  networks: { name: string; mac: string; addresses: string[]; rxRate: number; txRate: number; rxTotal: number; txTotal: number }[]
  sensors: { label: string; temperature: number }[]
  processes: {
    total: number
    matched: number
    list: { pid: number; parent: number | null; name: string; cpu: number; memory: number; status: string; user: string | null; runTime: number; exe: string | null }[]
  } | null
}

export type SortBy = 'cpu' | 'memory' | 'name' | 'pid'
