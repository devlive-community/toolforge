/** 导出用的独立 SVG：去掉交互层，把主题颜色、背景与字体写成具体值 */
export function svgMarkup(svg: SVGSVGElement) {
  const clone = svg.cloneNode(true) as SVGSVGElement
  clone.querySelectorAll('[data-export-skip]').forEach((node) => node.remove())
  const style = getComputedStyle(svg)
  const background = getComputedStyle(svg.closest('section') ?? document.body).backgroundColor
  clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg')
  clone.setAttribute('color', style.color)
  clone.setAttribute('font-family', style.fontFamily)
  clone.removeAttribute('class')
  const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect')
  rect.setAttribute('width', '100%')
  rect.setAttribute('height', '100%')
  rect.setAttribute('fill', background)
  clone.insertBefore(rect, clone.firstChild)
  return new XMLSerializer().serializeToString(clone)
}

/** 按 2 倍分辨率栅格化为 PNG 的 data URL */
export async function pngDataUrl(svg: SVGSVGElement, scale = 2) {
  const markup = svgMarkup(svg)
  const url = URL.createObjectURL(new Blob([markup], { type: 'image/svg+xml' }))
  try {
    const image = new Image()
    image.src = url
    await image.decode()
    const canvas = document.createElement('canvas')
    canvas.width = svg.width.baseVal.value * scale
    canvas.height = svg.height.baseVal.value * scale
    const context = canvas.getContext('2d')
    if (!context) throw new Error('canvas')
    context.scale(scale, scale)
    context.drawImage(image, 0, 0)
    return canvas.toDataURL('image/png')
  } finally {
    URL.revokeObjectURL(url)
  }
}
