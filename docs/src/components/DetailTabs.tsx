'use client'
import { useId, useRef, useState, type ReactNode } from 'react'

export function DetailTabs({ label, panels }: { label: string; panels: { id: string; title: string; content: ReactNode }[] }) {
  const [active, setActive] = useState(0)
  const id = useId()
  const buttons = useRef<(HTMLButtonElement | null)[]>([])
  return (
    <>
      <div className="site-tabs" role="tablist" aria-label={label}>
        {panels.map((panel, index) => (
          <button
            key={panel.id}
            type="button"
            id={`${id}-${panel.id}-tab`}
            role="tab"
            aria-controls={`${id}-${panel.id}`}
            aria-selected={active === index}
            tabIndex={active === index ? 0 : -1}
            onClick={() => setActive(index)}
            onKeyDown={(e) => {
              let next = index
              if (e.key === 'ArrowRight') next = (index + 1) % panels.length
              else if (e.key === 'ArrowLeft') next = (index - 1 + panels.length) % panels.length
              else if (e.key === 'Home') next = 0
              else if (e.key === 'End') next = panels.length - 1
              else return
              e.preventDefault()
              setActive(next)
              buttons.current[next]?.focus()
            }}
            ref={(element) => {
              buttons.current[index] = element
            }}
          >
            {panel.title}
          </button>
        ))}
      </div>
      {panels.map((panel, index) => (
        <div key={panel.id} id={`${id}-${panel.id}`} role="tabpanel" aria-labelledby={`${id}-${panel.id}-tab`} hidden={active !== index}>
          {panel.content}
        </div>
      ))}
    </>
  )
}
