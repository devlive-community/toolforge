import GuidePage, { generateMetadata as guideMetadata } from './[topic]/page'
import type { Lang } from '@/lib/catalog'
export async function generateMetadata({ params }: { params: Promise<{ lang: Lang }> }) {
  return guideMetadata({ params: params.then(({ lang }) => ({ lang, topic: 'start' })) })
}
export default async function GuideIndex({ params }: { params: Promise<{ lang: Lang }> }) {
  return GuidePage({ params: params.then(({ lang }) => ({ lang, topic: 'start' })) })
}
