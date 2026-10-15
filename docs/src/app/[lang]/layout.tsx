import type { Metadata, Viewport } from 'next'
import { IBM_Plex_Mono } from 'next/font/google'
import { notFound } from 'next/navigation'
import { Footer } from '@/components/Footer'
import { Header } from '@/components/Header'
import { LANGS, type Lang } from '@/lib/catalog'
import '../../styles/globals.css'

const plex = IBM_Plex_Mono({
  subsets: ['latin'],
  weight: ['400', '500', '600', '700'],
  variable: '--font-plex',
  display: 'swap',
})

// 与桌面端一致：默认暗色，偏好亮色的用户可通过右上角切换（存 localStorage）
const themeScript = `(function(){try{var t=localStorage.getItem('tf-theme');var d=t?t==='dark':true;document.documentElement.classList.toggle('dark',d)}catch(e){document.documentElement.classList.add('dark')}})()`

export function generateStaticParams() {
  return LANGS.map((lang) => ({ lang }))
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ lang: string }>
}): Promise<Metadata> {
  const { lang } = await params
  const zh = lang !== 'en'
  const title = 'ToolForge — ' + (zh ? '桌面工具箱，全部本地运行' : 'The desktop toolbox that stays local')
  const description = zh
    ? '65 个开箱即用的桌面工具插件：识别、转换、编码、网络、系统信息，全部在本地 Rust 中运行。'
    : '65 out-of-the-box desktop tools — OCR, converters, encoding, network and system info, all running locally in Rust.'
  const ogImage = { url: '/og-image.png', width: 1200, height: 630, alt: 'ToolForge' }
  return {
    title,
    description,
    metadataBase: new URL('https://toolforge.devlive.org'),
    alternates: { canonical: `/${zh ? 'zh' : 'en'}/` },
    icons: {
      icon: [
        { url: '/favicon.ico', sizes: '48x48' },
        { url: '/icon-192.png', sizes: '192x192', type: 'image/png' },
      ],
      apple: '/apple-touch-icon.png',
    },
    openGraph: {
      title,
      description,
      type: 'website',
      siteName: 'ToolForge',
      url: `https://toolforge.devlive.org/${zh ? 'zh' : 'en'}/`,
      images: [ogImage],
    },
    twitter: { card: 'summary_large_image', title, description, images: ['/og-image.png'] },
  }
}

export const viewport: Viewport = {
  themeColor: [
    { media: '(prefers-color-scheme: dark)', color: '#0b0d0e' },
    { media: '(prefers-color-scheme: light)', color: '#fafaf9' },
  ],
}

export default async function RootLayout({
  children,
  params,
}: {
  children: React.ReactNode
  params: Promise<{ lang: string }>
}) {
  const { lang } = await params
  if (!LANGS.includes(lang as Lang)) notFound()
  return (
    <html lang={lang} className={`${plex.variable} dark`} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeScript }} />
      </head>
      <body className="min-h-dvh">
        <Header lang={lang as Lang} />
        <main>{children}</main>
        <Footer lang={lang as Lang} />
      </body>
    </html>
  )
}
