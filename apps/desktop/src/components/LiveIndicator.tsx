import { useEffect, useState } from "react";
import { ArrowUpRight, Pause, Play, Square } from "lucide-react";
import { events, ipc, type IndicatorSnapshot } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useT } from "@/lib/i18n";

const clock = (secs: number) => {
  const h = Math.floor(secs / 3600);
  const m = Math.floor(secs / 60) % 60;
  const s = String(secs % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
};

// Indicador flotante de grabación, en su propia ventana arriba en el centro.
// Sale cuando se graba con la ventana principal en segundo plano: el rombo
// late con la voz, el tiempo de la sesión y pausar, parar y volver a la nota.
// Se arrastra por el rombo y el tiempo.
export function LiveIndicator() {
  const t = useT();
  const [snap, setSnap] = useState<IndicatorSnapshot | null>(null);
  const [level, setLevel] = useState(0);

  useEffect(() => {
    ipc.indicatorState().then(setSnap).catch(() => setSnap(null));
    const subs = [
      events.indicatorUpdate(setSnap),
      events.levels((l) => setLevel(Math.max(l.mic, l.system))),
    ];
    return () => subs.forEach((p) => p.then((f) => f()));
  }, []);

  if (!snap) return null;
  const paused = snap.phase === "paused";
  const db = 20 * Math.log10(Math.max(level, 1e-6));
  const h = paused ? 0 : Math.max(0, Math.min(1, (db + 60) / 60));

  return (
    <div className="live-indicator" data-phase={snap.phase}>
      <div className="li-drag" data-tauri-drag-region>
        <span className={cn("li-diamond", paused && "paused")} style={{ transform: `scale(${1 + 0.35 * h})` }} data-tauri-drag-region aria-hidden="true" />
        <span className="li-time" data-tauri-drag-region>
          {snap.phase === "starting" ? t("rec.preparing") : clock(snap.secs)}
        </span>
        {paused && (
          <span className="li-state" data-tauri-drag-region>
            {t("rec.paused")}
          </span>
        )}
      </div>
      {paused ? (
        <button className="li-btn" title={t("rec.resume")} aria-label={t("rec.resume")} onClick={() => ipc.indicatorPause(false)}>
          <Play className="size-[14px]" fill="currentColor" strokeWidth={0} />
        </button>
      ) : (
        <button className="li-btn" title={t("rec.pause")} aria-label={t("rec.pause")} disabled={snap.phase === "starting"} onClick={() => ipc.indicatorPause(true)}>
          <Pause className="size-[14px]" fill="currentColor" strokeWidth={0} />
        </button>
      )}
      <button className="li-btn stop" title={t("rec.stop")} aria-label={t("rec.stop")} onClick={() => ipc.indicatorStop()}>
        <Square className="size-[12px]" fill="currentColor" strokeWidth={0} />
      </button>
      <button className="li-btn" title={t("indicator.open")} aria-label={t("indicator.open")} onClick={() => ipc.indicatorOpen()}>
        <ArrowUpRight className="size-[15px]" />
      </button>
    </div>
  );
}
