import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

// Botón de shadcn/ui con nuestras variantes: píldora con borde fino
// (outline), oscura (primary), fantasma (ghost) y destructiva.
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-full text-[13.5px] font-medium transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/40 disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        outline: "border border-line bg-surface/70 text-ink hover:bg-hover",
        primary: "bg-ink text-canvas hover:bg-ink/85",
        olive: "bg-olive text-canvas hover:bg-olive/90",
        ghost: "text-ink-2 hover:bg-hover hover:text-ink",
        destructive: "bg-danger text-white hover:bg-danger/90",
      },
      size: {
        default: "h-8 px-3.5",
        sm: "h-7 px-3 text-[13px]",
        lg: "h-10 px-4",
        icon: "size-8",
        iconSm: "size-7",
      },
    },
    defaultVariants: { variant: "outline", size: "default" },
  },
);

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(({ className, variant, size, asChild = false, ...props }, ref) => {
  const Comp = asChild ? Slot : "button";
  return <Comp className={cn(buttonVariants({ variant, size, className }))} ref={ref} {...props} />;
});
Button.displayName = "Button";

export { buttonVariants };
