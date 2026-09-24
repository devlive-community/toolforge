export interface Subnet {
  cidr: string
  first: string
  last: string
}

export interface Report {
  version: 4 | 6
  address: string
  prefix: number
  cidr: string
  network: string
  broadcast: string | null
  netmask: string
  wildcard: string
  firstHost: string
  lastHost: string
  total: string
  usable: string
  kind: string
  class: string | null
  binary: string
  hex: string
  integer: string
  reverseDns: string
  expanded: string
  subnets: Subnet[]
  subnetCount: string | null
  membership: { address: string; inside: boolean } | null
}

export interface RangeReport {
  cidrs: string[]
  total: string
}
