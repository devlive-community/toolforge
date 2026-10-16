export type Mode = 'lines' | 'csv' | 'log'

export interface Hit {
  rule: string
  pattern: string
  matched: string
}

export interface Part {
  name: string
  version: string | null
  hit: Hit
}

export interface Bot extends Part {
  category: string
}

export type DeviceKind = 'bot' | 'tv' | 'console' | 'wearable' | 'tablet' | 'mobile' | 'desktop' | 'unknown'

export interface Parsed {
  browser: Part | null
  engine: Part | null
  os: Part | null
  device: { kind: DeviceKind; vendor: string | null; model: string | null; hit: Hit | null }
  bot: Bot | null
  notes: string[]
}

export interface Row extends Parsed {
  ua: string
  count: number
  line: number
}

export interface Bucket {
  name: string
  count: number
}

export interface Output {
  rulesVersion: string
  rows: Row[]
  truncated: boolean
  stats: {
    total: number
    unique: number
    empty: number
    bots: number
    browsers: Bucket[]
    systems: Bucket[]
    devices: Bucket[]
    botNames: Bucket[]
    botCategories: Bucket[]
  }
  columns: string[]
  skipped: number
}

export const SAMPLE_UA =
  'Mozilla/5.0 (Linux; Android 14; SM-S9180) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Mobile Safari/537.36'

export const SAMPLE_LOG = [
  '203.0.113.7 - - [16/Oct/2026:10:00:00 +0800] "GET / HTTP/1.1" 200 5120 "-" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36"',
  '198.51.100.4 - - [16/Oct/2026:10:00:02 +0800] "GET /docs HTTP/1.1" 200 2048 "https://example.org/" "Mozilla/5.0 (iPhone; CPU iPhone OS 17_6_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Mobile/15E148 Safari/604.1"',
  '66.249.66.1 - - [16/Oct/2026:10:00:03 +0800] "GET /robots.txt HTTP/1.1" 200 64 "-" "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)"',
  '192.0.2.10 - - [16/Oct/2026:10:00:05 +0800] "GET /api HTTP/1.1" 401 0 "-" "curl/8.7.1"',
  '203.0.113.7 - - [16/Oct/2026:10:00:09 +0800] "GET /about HTTP/1.1" 200 1024 "https://example.org/" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36"',
].join('\n')
