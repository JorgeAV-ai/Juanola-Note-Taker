import { useEffect, useState, type ReactNode } from "react";
import { AudioLines, BellRing, BookA, Boxes, CalendarDays, ChevronLeft, FolderOpen, Globe, HardDrive, House, Info, Mic, SlidersHorizontal, Speaker } from "lucide-react";
import type { Route } from "@/App";
import { ipc, type AudioDevice, type CalendarAccess, type DetectionSettings, type Permission } from "@/lib/ipc";
import { LANGUAGES, saveLanguage, savedLanguage } from "@/lib/language";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { ModelActions, ModelList } from "@/components/ModelsDialog";
import { useModels } from "@/state/models";

export type SettingsSection = "preferences" | "audio" | "models" | "calendar" | "data";

const SECTIONS: [SettingsSection, string, ReactNode][] = [
  ["preferences", "Preferences", <SlidersHorizontal key="p" />],
  ["audio", "Audio", <AudioLines key="a" />],
  ["models", "Models", <Boxes key="m" />],
  ["calendar", "Calendar", <CalendarDays key="c" />],
  ["data", "Data", <HardDrive key="d" />],
];

const MIC_TEXT: Record<Permission, string> = {
  granted: "Concedido",
  denied: "Denegado: actívalo en Ajustes del Sistema",
  not_determined: "Sin decidir: se pedirá al pulsar Start",
  unknown: "Desconocido",
};

// Ajustes con la disposición de Granola: navegación propia a la izquierda,
// botón para volver a Home, título en serif y tarjetas con filas.
export function SettingsView({ section, onNavigate }: { section: SettingsSection; onNavigate: (r: Route) => void }) {
  return (
    <div className="settings flex h-full bg-canvas">
      <aside className="flex w-[248px] shrink-0 flex-col border-r border-line bg-sidebar px-2.5">
        <div className="h-12 shrink-0" data-tauri-drag-region />
        <div className="mb-4 flex justify-center">
          <span className="flex size-12 items-center justify-center rounded-full border border-line bg-hover font-serif text-[20px] text-ink-2">M</span>
        </div>
        {SECTIONS.map(([key, label, icon]) => (
          <button
            key={key}
            className={cn(
              "settings-nav sidebar-item flex h-9 items-center gap-2.5 rounded-xl px-2.5 text-[14.5px] hover:bg-hover [&_svg]:size-[17px] [&_svg]:text-ink-2",
              section === key && "active bg-active",
            )}
            onClick={() => onNavigate({ kind: "settings", section: key })}
          >
            {icon}
            {label}
          </button>
        ))}
      </aside>
      <main className="flex min-w-0 flex-1 flex-col">
        <div className="relative flex h-12 shrink-0 items-center px-3" data-tauri-drag-region>
          <button
            className="back-home flex h-8 items-center gap-0.5 rounded-full border border-line px-2.5 text-ink-2 hover:bg-hover"
            aria-label="Volver a Home"
            onClick={() => onNavigate({ kind: "home" })}
          >
            <ChevronLeft className="size-4" />
            <House className="size-4" />
          </button>
          <span className="pointer-events-none absolute inset-x-0 text-center text-[14px] text-ink-2">Settings</span>
        </div>
        <div className="min-h-0 flex-1 overflow-auto">
          <div className="mx-auto max-w-[760px] px-10 pb-16">
            {section === "preferences" && <Preferences />}
            {section === "audio" && <Audio />}
            {section === "models" && <Models />}
            {section === "calendar" && <Calendar />}
            {section === "data" && <Data />}
          </div>
        </div>
      </main>
    </div>
  );
}

function Page({ title, children }: { title: string; children: ReactNode }) {
  return (
    <>
      <h1 className="page-title mb-8 mt-6 font-serif text-[34px] font-normal leading-tight">{title}</h1>
      {children}
    </>
  );
}

function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section className="settings-group mb-8">
      <h2 className="mb-2.5 text-[14px] text-muted">{label}</h2>
      <div className="divide-y divide-line rounded-[20px] border border-line bg-card">{children}</div>
    </section>
  );
}

function Row({ icon, title, desc, children, testId }: { icon: ReactNode; title: string; desc?: ReactNode; children?: ReactNode; testId?: string }) {
  return (
    <div className="settings-row flex items-center gap-4 px-5 py-4">
      <span className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-line bg-canvas text-ink-2 [&_svg]:size-[18px]">{icon}</span>
      <span className="min-w-0 flex-1">
        <span className="block text-[15px]">{title}</span>
        {desc && (
          <span className="mt-0.5 block text-[13.5px] leading-snug text-muted" data-testid={testId}>
            {desc}
          </span>
        )}
      </span>
      {children}
    </div>
  );
}

function useDetection() {
  const [settings, setSettings] = useState<DetectionSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    ipc.detectionSettings().then(setSettings).catch((e) => setError(String(e)));
  }, []);
  const update = (patch: Partial<DetectionSettings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    ipc.setDetectionSettings(next).catch((e) => setError(String(e)));
  };
  return { settings, update, error };
}

function Preferences() {
  const [language, setLanguage] = useState(savedLanguage);
  const detection = useDetection();
  return (
    <Page title="Preferences">
      {detection.error && <div className="note-error mb-4 text-[13.5px] text-danger">{detection.error}</div>}
      <Group label="Reuniones">
        <Row icon={<BellRing />} title="Avisar al detectar una llamada" desc="Cuando Zoom, Teams, Meet u otra app abre el micrófono, aparece «Meeting detected» con «Take Notes». Nunca graba sin que lo pulses.">
          <Switch
            className="detect-meetings"
            checked={detection.settings?.meetings ?? false}
            disabled={!detection.settings}
            onCheckedChange={(v) => detection.update({ meetings: v })}
            aria-label="Avisar al detectar una llamada"
          />
        </Row>
      </Group>
      <Group label="Transcripción">
        <Row icon={<Globe />} title="Idioma de las reuniones" desc="Se usa al pulsar Start; también se cambia en la píldora. «Detectar» fija el idioma tras unos segundos de voz.">
          <select
            className="settings-select h-9 shrink-0 cursor-pointer rounded-full border border-line bg-surface px-3 text-[14px] outline-none"
            value={language}
            onChange={(e) => {
              setLanguage(e.target.value);
              saveLanguage(e.target.value);
            }}
          >
            {LANGUAGES.map(([v, label]) => (
              <option key={v} value={v}>
                {label}
              </option>
            ))}
          </select>
        </Row>
        <Glossary />
      </Group>
    </Page>
  );
}

// Términos que el STT debe conocer: van en el prompt del modelo junto con el
// título, los asistentes y las notas de la reunión.
function Glossary() {
  const [text, setText] = useState<string | null>(null);
  const [saved, setSaved] = useState("");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    ipc
      .sttGlossary()
      .then((g) => {
        setText(g);
        setSaved(g);
      })
      .catch((e) => setError(String(e)));
  }, []);
  const save = () => {
    if (text === null || text === saved) return;
    ipc
      .setSttGlossary(text)
      .then(() => setSaved(text))
      .catch((e) => setError(String(e)));
  };
  return (
    <div className="settings-row px-5 py-4">
      <div className="flex items-center gap-4">
        <span className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-line bg-canvas text-ink-2 [&_svg]:size-[18px]">
          <BookA />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-[15px]">Vocabulario</span>
          <span className="mt-0.5 block text-[13.5px] leading-snug text-muted">
            Nombres de personas, empresas, productos y siglas que salen en tus reuniones, separados por comas o uno por línea. Ayuda a transcribirlos bien; el título, los asistentes y tus notas ya se usan solos.
          </span>
        </span>
      </div>
      <textarea
        className="stt-glossary mt-3 min-h-[88px] w-full resize-y rounded-xl border border-line bg-surface px-3 py-2 text-[14px] outline-none focus:border-ink-2"
        value={text ?? ""}
        disabled={text === null}
        placeholder="Rutario, SAP S/4HANA, Íñigo Etxeberria…"
        onChange={(e) => setText(e.target.value)}
        onBlur={save}
        aria-label="Vocabulario"
      />
      {error && <div className="note-error mt-2 text-[13px] text-danger">{error}</div>}
    </div>
  );
}

function Audio() {
  const [mic, setMic] = useState<Permission | null>(null);
  const [devices, setDevices] = useState<AudioDevice[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refreshMic = () => ipc.micPermissionStatus().then(setMic).catch(() => setMic("unknown"));
  useEffect(() => {
    refreshMic();
    ipc.listAudioDevices().then(setDevices).catch(() => setDevices([]));
    // Al volver de Ajustes del Sistema, el permiso puede haber cambiado.
    window.addEventListener("focus", refreshMic);
    return () => window.removeEventListener("focus", refreshMic);
  }, []);
  const open = (kind: "mic_settings" | "audio_capture_settings") => ipc.openPrivacySettings(kind).catch((e) => setError(String(e)));

  return (
    <Page title="Audio">
      {error && <div className="note-error mb-4 text-[13.5px] text-danger">{error}</div>}
      <Group label="Permisos de macOS">
        <Row icon={<Mic />} title="Micrófono" desc={mic ? MIC_TEXT[mic] : "…"} testId="mic-permission">
          {mic === "granted" ? <span className="text-[14px] text-olive-ink">✓ Enabled</span> : null}
          <Button onClick={() => open("mic_settings")}>Abrir Ajustes</Button>
        </Row>
        <Row icon={<Speaker />} title="Audio del sistema" desc="Para oír a los demás. macOS no deja consultarlo: si «Them» sale vacío, revísalo aquí.">
          <Button onClick={() => open("audio_capture_settings")}>Abrir Ajustes</Button>
        </Row>
      </Group>
      <Group label="Dispositivos de entrada">
        {devices === null ? (
          <div className="px-5 py-4 text-muted">…</div>
        ) : devices.length === 0 ? (
          <div className="px-5 py-4 text-muted">No se encontró ningún micrófono.</div>
        ) : (
          <ul className="settings-list m-0 list-none p-0">
            {devices.map((d) => (
              <li key={d.id} className="flex items-center gap-3 px-5 py-3 text-[14.5px]">
                <Mic className="size-4 text-ink-2" />
                <span className="flex-1">{d.name}</span>
                {d.is_bluetooth && <span className="tag rounded-full border border-line px-2 py-0.5 text-[12px] text-muted">Bluetooth</span>}
                {d.is_default && <span className="tag rounded-full bg-olive/10 px-2 py-0.5 text-[12px] text-olive-ink">Predeterminado</span>}
              </li>
            ))}
          </ul>
        )}
      </Group>
      <p className="-mt-5 px-1 text-[13px] text-muted">Juanola graba del micrófono predeterminado; se cambia en Ajustes del Sistema › Sonido.</p>
    </Page>
  );
}

const CALENDAR_TEXT: Record<CalendarAccess, string> = {
  granted: "Conectado. «Coming up» muestra los eventos de todas las cuentas de la app Calendario (iCloud, Google, Exchange…).",
  not_determined: "Sin conectar. Juanola lee el calendario de macOS; añade ahí tus cuentas de Google o Microsoft.",
  denied: "Acceso denegado. Actívalo en Ajustes del Sistema › Privacidad › Calendarios.",
  write_only: "Solo escritura: Juanola necesita leer los eventos. Cámbialo a «Acceso total» en Ajustes del Sistema.",
  unsupported: "El calendario solo está disponible en macOS por ahora.",
};

function Calendar() {
  const [access, setAccess] = useState<CalendarAccess | null>(null);
  const [error, setError] = useState<string | null>(null);
  const detection = useDetection();
  const refresh = () => ipc.calendarAccess().then(setAccess).catch(() => setAccess("unsupported"));
  useEffect(() => {
    refresh();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, []);
  const connect = () =>
    ipc
      .requestCalendarAccess()
      .then(setAccess)
      .catch((e) => setError(String(e)));
  const openSettings = () =>
    ipc.openPrivacySettings("calendar_settings").catch((e) => setError(String(e)));

  return (
    <Page title="Calendar">
      {(error || detection.error) && <div className="note-error mb-4 text-[13.5px] text-danger">{error ?? detection.error}</div>}
      <Group label="Calendario de macOS">
        <Row icon={<CalendarDays />} title="Calendario" desc={access ? CALENDAR_TEXT[access] : "…"} testId="calendar-access">
          {access === "granted" ? (
            <span className="text-[14px] text-olive-ink">✓ Enabled</span>
          ) : access === "not_determined" ? (
            <Button className="calendar-connect" onClick={connect}>
              Conectar
            </Button>
          ) : access === "denied" || access === "write_only" ? (
            <Button onClick={openSettings}>Abrir Ajustes</Button>
          ) : null}
        </Row>
      </Group>
      <Group label="Avisos">
        <Row icon={<BellRing />} title="Avisar 1 minuto antes" desc="Para eventos con dos o más asistentes. «Take Notes» abre la videollamada y empieza a grabar.">
          <Switch
            className="calendar-reminders"
            checked={detection.settings?.calendar_reminders ?? false}
            disabled={!detection.settings}
            onCheckedChange={(v) => detection.update({ calendar_reminders: v })}
            aria-label="Avisar 1 minuto antes"
          />
        </Row>
        <Row icon={<BellRing />} title="Probar aviso" desc="Muestra un aviso de ejemplo arriba a la derecha.">
          <Button className="test-notice" onClick={() => ipc.simulateDetection("calendar").catch((e) => setError(String(e)))}>
            Calendario
          </Button>
          <Button className="test-notice-adhoc" onClick={() => ipc.simulateDetection("adhoc").catch((e) => setError(String(e)))}>
            Llamada
          </Button>
        </Row>
      </Group>
    </Page>
  );
}

function Models() {
  const { refresh, error } = useModels();
  useEffect(() => {
    refresh();
  }, [refresh]);
  return (
    <Page title="Models">
      <Group label="En este Mac">
        <ModelList />
      </Group>
      {error && <p className="models-error -mt-4 mb-4 text-[13.5px] text-danger">{error}</p>}
      <div className="flex justify-end gap-2">
        <ModelActions />
      </div>
      <p className="mt-6 text-[13.5px] leading-relaxed text-muted">
        Se descargan de Hugging Face y se comprueban con su SHA-256. Si se corta la conexión o pausas la descarga, continúa donde se quedó.
      </p>
    </Page>
  );
}

function Data() {
  const [info, setInfo] = useState<{ version: string; data_dir: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    ipc.appInfo().then(setInfo).catch(() => setInfo(null));
  }, []);
  return (
    <Page title="Data">
      {error && <div className="note-error mb-4 text-[13.5px] text-danger">{error}</div>}
      <Group label="Almacenamiento">
        <Row icon={<FolderOpen />} title="Carpeta de datos" desc={<span className="mono break-all font-mono text-[12.5px]">{info?.data_dir ?? "…"}</span>}>
          <Button onClick={() => ipc.revealDataDir().catch((e) => setError(String(e)))}>Mostrar en Finder</Button>
        </Row>
        <Row icon={<Info />} title="Todo se queda en este Mac" desc={`Notas, transcripciones y modelos. Versión ${info?.version ?? "…"}.`} />
      </Group>
    </Page>
  );
}
