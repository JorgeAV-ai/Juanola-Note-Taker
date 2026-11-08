import { Check, Download } from "lucide-react";
import { Logo } from "./Logo";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { formatBytes, missing, SLOT_LABEL, useModels } from "@/state/models";

/** Lista de modelos con su estado y, si se está descargando, el progreso. */
export function ModelList() {
  const { models, installing, progress } = useModels();
  return (
    <ul className="model-list divide-y divide-line">
      {models.map((m) => {
        const active = installing && progress?.slot === m.slot;
        const pct = active && progress ? Math.round((100 * progress.done) / Math.max(progress.total, 1)) : 0;
        return (
          <li key={m.slot} className="model-row flex items-center gap-4 px-5 py-3.5" data-slot={m.slot} data-installed={m.installed}>
            <span className="min-w-0 flex-1">
              <span className="block text-[15px]">{SLOT_LABEL[m.slot] ?? m.slot}</span>
              <span className="mt-0.5 block truncate text-[13px] text-muted">
                {m.repo} · {m.size_mb ? formatBytes(m.size_mb * 1e6) : "?"}
              </span>
              {active && progress && (
                <span className="mt-2 flex items-center gap-2.5">
                  <span className="model-progress h-1.5 flex-1 overflow-hidden rounded-full bg-hover">
                    <span className="block h-full rounded-full bg-olive transition-[width]" style={{ width: `${pct}%` }} />
                  </span>
                  <span className="w-[110px] text-right text-[12.5px] tabular-nums text-muted">
                    {formatBytes(progress.done)} / {formatBytes(progress.total)}
                  </span>
                </span>
              )}
            </span>
            {m.installed ? (
              <span className="flex items-center gap-1 text-[13.5px] text-olive-ink">
                <Check className="size-4" /> Instalado
              </span>
            ) : active ? (
              <span className="text-[13.5px] tabular-nums text-ink-2">{pct} %</span>
            ) : (
              <span className="text-[13.5px] text-muted">{installing ? "En cola" : m.partial_bytes ? "A medias" : "Falta"}</span>
            )}
          </li>
        );
      })}
    </ul>
  );
}

/** Botones Descargar / Cancelar según el estado. */
export function ModelActions({ onLater }: { onLater?: () => void }) {
  const { models, installing, install, cancel } = useModels();
  const todo = missing(models);
  const total = todo.reduce((n, m) => n + (m.size_mb ?? 0) * 1e6 - m.partial_bytes, 0);
  if (installing) return <Button onClick={cancel}>Pausar descarga</Button>;
  if (!todo.length) return onLater ? <Button variant="primary" onClick={onLater}>Listo</Button> : null;
  return (
    <>
      {onLater && (
        <Button variant="ghost" onClick={onLater}>
          Más tarde
        </Button>
      )}
      <Button variant="primary" className="models-download" onClick={install}>
        <Download /> Descargar {formatBytes(Math.max(total, 0))}
      </Button>
    </>
  );
}

// Se abre al arrancar si faltan modelos y cuando se intenta grabar o generar
// notas sin ellos. La descarga sigue aunque se cierre.
export function ModelsDialog() {
  const { dialogOpen, openDialog, models, error } = useModels();
  const done = models.length > 0 && missing(models).length === 0;
  return (
    <Dialog open={dialogOpen} onOpenChange={openDialog}>
      <DialogContent className="models-dialog top-[18vh] w-[min(560px,calc(100vw-48px))]">
        <div className="flex items-start gap-4 px-6 pb-2 pt-6">
          <Logo className="mt-1 h-9 shrink-0" />
          <div>
            <DialogTitle className="font-serif text-[24px] font-normal leading-tight">
              {done ? "Modelos listos" : "Descarga los modelos"}
            </DialogTitle>
            <DialogDescription className="mt-1.5 text-[14px] leading-snug text-ink-2">
              {done
                ? "Ya puedes grabar y generar notas sin conexión."
                : "Juanola transcribe y resume en este Mac, sin enviar nada a internet. Para eso necesita estos modelos de Hugging Face; se bajan una sola vez."}
            </DialogDescription>
          </div>
        </div>
        <div className="mx-6 my-3 overflow-hidden rounded-2xl border border-line bg-card">
          <ModelList />
        </div>
        {error && <p className="models-error mx-6 mb-1 text-[13.5px] text-danger">{error}</p>}
        <div className="flex justify-end gap-2 px-6 pb-6 pt-2">
          <ModelActions onLater={() => openDialog(false)} />
        </div>
      </DialogContent>
    </Dialog>
  );
}
