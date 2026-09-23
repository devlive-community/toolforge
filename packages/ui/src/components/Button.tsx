import { forwardRef, type ButtonHTMLAttributes } from 'react'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '../utils'
import { Spinner } from './Spinner'

export const buttonVariants = cva(
  [
    'inline-flex shrink-0 items-center justify-center whitespace-nowrap rounded-control font-medium select-none',
    'transition-colors duration-150 outline-none focus-visible:ring-2 focus-visible:ring-ring',
    'disabled:pointer-events-none disabled:opacity-50 [&_svg]:shrink-0',
  ],
  {
    variants: {
      variant: {
        primary: 'bg-primary text-fg-on-primary hover:bg-primary-hover',
        secondary: 'bg-surface-2 text-fg border border-border hover:bg-hover',
        outline: 'bg-surface text-fg border border-border hover:border-border-strong hover:bg-hover',
        ghost: 'bg-transparent text-fg-muted hover:bg-hover hover:text-fg',
        soft: 'bg-primary-soft text-primary-fg hover:brightness-95',
        danger: 'bg-danger text-fg-on-primary hover:opacity-90',
      },
      size: {
        sm: 'h-control-sm gap-1.5 px-2.5 text-xs [&_svg]:size-3.5',
        md: 'h-control-md gap-1.5 px-3 text-[13px] [&_svg]:size-4',
        lg: 'h-control-lg gap-2 px-4 text-sm [&_svg]:size-4',
        'icon-sm': 'size-control-sm [&_svg]:size-3.5',
        'icon-md': 'size-control-md [&_svg]:size-4',
        'icon-lg': 'size-control-lg [&_svg]:size-[18px]',
      },
      block: { true: 'w-full' },
    },
    defaultVariants: { variant: 'outline', size: 'md' },
  },
)

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  loading?: boolean
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, block, loading, disabled, children, type = 'button', ...props }, ref) => (
    <button
      ref={ref}
      type={type}
      className={cn(buttonVariants({ variant, size, block }), className)}
      disabled={disabled || loading}
      {...props}
    >
      {loading && <Spinner />}
      {children}
    </button>
  ),
)
Button.displayName = 'Button'
