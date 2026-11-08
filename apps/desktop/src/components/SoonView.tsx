// Secciones de Granola que todavía no tienen nada detrás.
export type SoonKind = "shared" | "recipes" | "people" | "companies";

const TEXT: Record<SoonKind, [string, string]> = {
  shared: [
    "Shared with me",
    "Las notas que otros comparten contigo necesitan cuentas y sincronización entre equipos. Juanola funciona en local, sin servidor.",
  ],
  recipes: [
    "Recipes",
    "Las recetas son prompts guardados que se lanzan sobre una o varias reuniones («prepárame el seguimiento», «resume las decisiones»). Llegarán con el chat, que aún no está conectado al modelo.",
  ],
  people: ["People", "Personas a partir de los asistentes de cada reunión. Depende de la integración con el calendario, que todavía no está hecha."],
  companies: ["Companies", "Empresas a partir de los dominios de los asistentes. Depende de la integración con el calendario, que todavía no está hecha."],
};

export function SoonView({ what }: { what: SoonKind }) {
  const [title, body] = TEXT[what];
  return (
    <div className="soon mx-auto max-w-[760px] px-10">
      <h1 className="page-title mb-3 mt-6 font-serif text-[34px] font-normal leading-tight">{title}</h1>
      <p className="soon-badge mb-5 inline-block rounded-full border border-line px-3 py-1 text-[12.5px] text-muted">Todavía no disponible</p>
      <p className="max-w-[560px] text-[15px] leading-relaxed text-ink-2">{body}</p>
    </div>
  );
}
