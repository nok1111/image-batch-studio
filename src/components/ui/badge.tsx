import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-md px-2 py-0.5 text-[11px] font-medium",
  {
    variants: {
      variant: {
        accent: "bg-accent/15 text-accent",
        teal: "bg-accent-2/15 text-accent-2",
        muted: "bg-bg-secondary text-text-secondary",
        warning: "bg-warning/15 text-warning",
        success: "bg-success/15 text-success",
        danger: "bg-danger/15 text-danger",
      },
    },
    defaultVariants: { variant: "muted" },
  },
);

export function Badge({
  className,
  variant,
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & VariantProps<typeof badgeVariants>) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}
