export interface Name {
  commonName: string | null
  organization: string | null
  dn: string
}

export interface Certificate {
  subject: Name
  issuer: Name
  serial: string
  version: number
  notBefore: string
  notAfter: string
  daysLeft: number
  validity: 'valid' | 'expired' | 'not_yet_valid'
  altNames: { kind: 'dns' | 'ip' | 'email' | 'uri'; value: string }[]
  key: { algorithm: string; bits: number | null; curve: string | null }
  signatureAlgorithm: string
  selfSigned: boolean
  isCa: boolean
  pathLen: number | null
  keyUsage: string[]
  extendedKeyUsage: string[]
  subjectKeyId: string | null
  authorityKeyId: string | null
  ocsp: string[]
  caIssuers: string[]
  crl: string[]
  fingerprints: { sha1: string; sha256: string }
  pem: string
}

export interface Parsed {
  certificates: Certificate[]
  ordered: boolean
}

export type Verdict = 'trusted' | 'expired' | 'not_yet_valid' | 'name_mismatch' | 'untrusted' | 'revoked' | 'invalid'

export interface FetchReport extends Parsed {
  host: string
  port: number
  address: string
  protocol: string | null
  cipher: string | null
  verdict: Verdict
  verdictDetail: string | null
  fakeIp: boolean
  elapsedMs: number
}
