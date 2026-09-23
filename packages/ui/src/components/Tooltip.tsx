import { cloneElement, isValidElement, useState, type ReactElement, type ReactNode } from 'react'
import {
  autoUpdate,
  flip,
  FloatingPortal,
  offset,
  shift,
  useDismiss,
  useFloating,
  useFocus,
  useHover,
  useInteractions,
  useRole,
  type Placement,
} from '@floating-ui/react'

export interface TooltipProps {
  content: ReactNode
  children: ReactElement<Record<string, unknown>>
  placement?: Placement
  delay?: number
}

export function Tooltip({ content, children, placement = 'bottom', delay = 400 }: TooltipProps) {
  const [open, setOpen] = useState(false)
  const { refs, floatingStyles, context } = useFloating({
    open,
    onOpenChange: setOpen,
    placement,
    whileElementsMounted: autoUpdate,
    middleware: [offset(6), flip({ padding: 8 }), shift({ padding: 8 })],
  })
  const { getReferenceProps, getFloatingProps } = useInteractions([
    useHover(context, { delay: { open: delay, close: 0 }, move: false }),
    useFocus(context),
    useDismiss(context),
    useRole(context, { role: 'tooltip' }),
  ])

  if (!content) return children

  return (
    <>
      {isValidElement(children) &&
        cloneElement(children, { ref: refs.setReference, ...getReferenceProps(children.props) })}
      {open && (
        <FloatingPortal>
          <div
            ref={refs.setFloating}
            style={floatingStyles}
            {...getFloatingProps()}
            className="z-tooltip pointer-events-none max-w-64 rounded-[6px] bg-fg px-2 py-1 text-xs text-bg shadow-popover"
          >
            {content}
          </div>
        </FloatingPortal>
      )}
    </>
  )
}
