import * as SliderPrimitive from "@radix-ui/react-slider";
import { cn } from "@/lib/utils";

interface SliderProps {
  value: number;
  min: number;
  max: number;
  step?: number;
  onValueChange: (value: number) => void;
  onValueCommit?: (value: number) => void;
  className?: string;
  disabled?: boolean;
  id?: string;
  "aria-label"?: string;
}

export function Slider({ value, onValueChange, onValueCommit, className, ...props }: SliderProps) {
  return (
    <SliderPrimitive.Root
      value={[value]}
      onValueChange={([v]) => v !== undefined && onValueChange(v)}
      onValueCommit={([v]) => v !== undefined && onValueCommit?.(v)}
      min={props.min}
      max={props.max}
      step={props.step ?? 1}
      disabled={props.disabled}
      className={cn("relative flex h-5 w-full touch-none select-none items-center", className)}
    >
      <SliderPrimitive.Track className="relative h-1.5 grow overflow-hidden rounded-full bg-bg-secondary">
        <SliderPrimitive.Range className="absolute h-full bg-accent" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb
        id={props.id}
        aria-label={props["aria-label"]}
        className="block size-4 rounded-full border-2 border-accent bg-text shadow transition-transform hover:scale-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60"
      />
    </SliderPrimitive.Root>
  );
}
