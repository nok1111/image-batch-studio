import { cn } from "@/lib/utils";
import type { SelectOption } from "./select";

interface SegmentedProps<T extends string> {
  value: T;
  options: SelectOption<T>[];
  onValueChange: (value: T) => void;
  "aria-label"?: string;
}

export function Segmented<T extends string>({ value, options, onValueChange, ...rest }: SegmentedProps<T>) {
  return (
    <div
      role="radiogroup"
      aria-label={rest["aria-label"]}
      className="inline-flex rounded-lg border border-border bg-bg-secondary p-0.5"
    >
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={o.value === value}
          onClick={() => onValueChange(o.value)}
          className={cn(
            "h-8 rounded-md px-3 text-xs font-medium transition-colors cursor-pointer",
            o.value === value ? "bg-card text-text shadow-card" : "text-text-secondary hover:text-text",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
