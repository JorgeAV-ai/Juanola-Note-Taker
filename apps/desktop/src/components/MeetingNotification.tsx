import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { events, ipc, type MeetingNotice } from "@/lib/ipc";
import { Logo } from "@/components/Logo";
import { cn } from "@/lib/utils";
import { tBackend, useT } from "@/lib/i18n";

const minutes = (ms: number) => Math.round(ms / 60_000);

// Título y texto del aviso en el idioma de la interfaz (el backend los manda en
// inglés; aquí se rehacen con el tipo de app y la hora del evento).
function texts(n: MeetingNotice, t: ReturnType<typeof useT>): [string, string] {
  if (n.kind === "calendar" && n.event) {
    const secs = (new Date(n.event.start).getTime() - Date.now()) / 1000;
    const body = secs > 0 ? t("notice.startsIn", { n: Math.ceil(secs / 60) }) : secs > -60 ? t("notice.startingNow") : t("notice.startedAgo", { n: minutes(-secs * 1000) });
    return [tBackend(n.title), body];
  }
  const kind = n.app?.kind;
  const title = kind === "slack_huddle" ? t("notice.huddle") : kind === "face_time" || kind === "whats_app" ? t("notice.call") : t("notice.meeting");
  return [title, n.body];
}

// Aviso de reunión de Granola, en su propia ventana arriba a la derecha: barra
// sólida a la izquierda si viene del calendario, discontinua si es una llamada
// detectada por el micrófono. «Take Notes» abre la nota y empieza a grabar.
export function MeetingNotification() {
  const [notice, setNotice] = useState<MeetingNotice | null>(null);
  const [busy, setBusy] = useState(false);
  const t = useT();

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
  const [title, body] = texts(notice, t);
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
        <p className="notice-title truncate text-[14.5px] font-semibold leading-snug">{title}</p>
        <p className="notice-body truncate text-[13px] leading-snug text-muted">{body}</p>
        <button className="notice-take mt-2.5 h-8 rounded-full bg-ink px-3.5 text-[13.5px] font-medium text-canvas disabled:opacity-60" onClick={take} disabled={busy}>
          {t("notice.take")}
        </button>
      </div>
      <button className="notice-close -mr-1 -mt-1 flex size-7 items-center justify-center rounded-full text-muted hover:bg-hover hover:text-ink" aria-label={t("common.close")} onClick={() => ipc.notificationDismiss()}>
        <X className="size-4" />
      </button>
    </div>
  );
}
