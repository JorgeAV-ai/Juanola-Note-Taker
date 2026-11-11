import { useState } from "react";
import { SquareSlash } from "lucide-react";
import { cn } from "@/lib/utils";
import { useT } from "@/lib/i18n";

// Barra «Ask anything» de Granola: píldora grande abajo, con una receta a la
// derecha. Al enviar abre el Chat con la pregunta (el chat aún no tiene modelo).
export function AskAnythingBar({
  onAsk,
  className,
  placeholder,
  recipe,
}: {
  onAsk: (q: string) => void;
  className?: string;
  placeholder?: string;
  recipe?: string;
}) {
  const t = useT();
  placeholder ??= t("ask.placeholder");
  recipe ??= t("ask.recipe");
  const [q, setQ] = useState("");
  return (
    <form
      className={cn("ask-bar flex h-[58px] items-center gap-2 rounded-full border border-line bg-surface pl-6 pr-2 shadow-[0_2px_12px_rgba(0,0,0,0.04)]", className)}
      onSubmit={(e) => {
        e.preventDefault();
        if (q.trim()) onAsk(q.trim());
      }}
    >
      <input className="ask-input min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-muted" placeholder={placeholder} value={q} onChange={(e) => setQ(e.target.value)} />
      <button
        type="button"
        onClick={() => onAsk(recipe!)}
        className="flex h-[42px] items-center gap-2 rounded-full border border-line px-4 text-[14.5px] text-ink transition-colors hover:bg-hover"
      >
        <SquareSlash className="size-4 text-ink-2" /> {recipe}
      </button>
    </form>
  );
}
