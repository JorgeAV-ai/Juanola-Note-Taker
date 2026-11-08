import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { events, ipc, type MeetingNotice } from "@/lib/ipc";
import { Logo } from "@/components/Logo";
import { cn } from "@/lib/utils";

// Aviso de reunión de Granola, en su propia ventana arriba a la derecha: barra
// sólida a la izquierda si viene del calendario, discontinua si es una llamada
// detectada por el micrófono. «Take Notes» abre la nota y empieza a grabar.
export function MeetingNotification() {
  const [notice, setNotice] = useState<MeetingNotice | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    ipc.notificationPayload().then(setNotice).catch(() => setNotice(null));
    const un = events.notificationUpdate((n) => {
      setNotice(n);
      setBusy(false);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  if (!notice) return null;
  const take = async () => {
    setBusy(true);
    try {
      await ipc.notificationTakeNotes();
    } catch {
      setBusy(false);
    }
  };

  return (
    <div className="meeting-notice" data-kind={notice.kind}>
      <span className={cn("notice-bar", notice.kind === "calendar" ? "solid" : "dashed")} aria-hidden="true" />
      {/* Sobre papel crema, como en la barra lateral: así se ve también en oscuro. */}
      <span className="flex size-8 shrink-0 items-center justify-center rounded-lg border border-line bg-[#f6f3ec] dark:border-transparent">
        <Logo className="h-5" />
      </span>
      <div className="min-w-0 flex-1">
        <p className="notice-title truncate text-[14.5px] font-semibold leading-snug">{notice.title}</p>
        <p className="notice-body truncate text-[13px] leading-snug text-muted">{notice.body}</p>
        <button className="notice-take mt-2.5 h-8 rounded-full bg-ink px-3.5 text-[13.5px] font-medium text-canvas disabled:opacity-60" onClick={take} disabled={busy}>
          Take Notes
        </button>
      </div>
      <button className="notice-close -mr-1 -mt-1 flex size-7 items-center justify-center rounded-full text-muted hover:bg-hover hover:text-ink" aria-label="Cerrar" onClick={() => ipc.notificationDismiss()}>
        <X className="size-4" />
      </button>
    </div>
  );
}
