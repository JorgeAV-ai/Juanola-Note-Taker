import { cn } from "@/lib/utils";

// Rombo vertical, como una pastilla Juanola: algo irregular (se cortan a mano)
// y con un brillo. El mismo dibujo que docs/assets/logo.svg y el icono.
export function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 64 100" className={cn("logo", className)} aria-hidden="true">
      <path d="M32 5 L58 49 L33 95 L6 51 Z" fill="var(--licorice)" stroke="var(--licorice)" strokeWidth="6" strokeLinejoin="round" />
      <path d="M30 18 L40 35" stroke="#fff" strokeOpacity=".22" strokeWidth="4" strokeLinecap="round" />
    </svg>
  );
}
