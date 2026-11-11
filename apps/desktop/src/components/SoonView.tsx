import { useT } from "@/lib/i18n";
import type { Key } from "@/lib/strings";


// Secciones de Granola que todavía no tienen nada detrás.
export type SoonKind = "shared" | "recipes" | "people" | "companies";

const TEXT: Record<SoonKind, [Key, Key]> = {
  shared: ["nav.shared", "soon.shared"],
  recipes: ["nav.recipes", "soon.recipes"],
  people: ["nav.people", "soon.people"],
  companies: ["nav.companies", "soon.companies"],
};

export function SoonView({ what }: { what: SoonKind }) {
  const t = useT();
  const [title, body] = TEXT[what].map((k) => t(k));
  return (
    <div className="soon mx-auto max-w-[760px] px-10">
      <h1 className="page-title mb-3 mt-6 font-serif text-[34px] font-normal leading-tight">{title}</h1>
      <p className="soon-badge mb-5 inline-block rounded-full border border-line px-3 py-1 text-[12.5px] text-muted">{t("common.soon")}</p>
      <p className="max-w-[560px] text-[15px] leading-relaxed text-ink-2">{body}</p>
    </div>
  );
}
