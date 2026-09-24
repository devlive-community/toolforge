export interface Swatch {
  hex: string
  percent: number
}

export interface Gps {
  latitude: number
  longitude: number
  altitude: number | null
}

export interface Summary {
  make: string | null
  model: string | null
  lens: string | null
  takenAt: string | null
  exposure: string | null
  aperture: string | null
  iso: string | null
  focalLength: string | null
  flash: string | null
  software: string | null
  orientation: number | null
  gps: Gps | null
}

export interface Entry {
  ifd: string
  tag: string
  value: string
}

export interface Report {
  path: string
  name: string
  size: number
  format: string | null
  width: number
  height: number
  aspect: [number, number]
  colorType: string
  hasAlpha: boolean
  bitsPerPixel: number
  preview: string | null
  palette: Swatch[]
  average: string | null
  summary: Summary | null
  exif: Entry[]
  canStrip: boolean
}

export interface StripReport {
  output: string
  removed: string[]
  savedBytes: number
  orientationLost: boolean
}
