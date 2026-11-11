import { useCallback, useEffect, useState } from "react";
import { CalendarClock, ChevronLeft, ChevronRight, UsersRound, Video } from "lucide-react";
import type { Route } from "@/App";
import { events as appEvents, ipc, type CalendarAccess, type CalendarEvent, type Note } from "@/lib/ipc";
import { timeLabel } from "@/lib/dates";
import { dateLocale, useT } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import { NoteList } from "@/components/NoteList";
import { AskAnythingBar } from "@/components/AskAnythingBar";

const PAGE = 5;

// Inicio: la agenda del calendario y las notas por día. Con
// `space`, la vista del espacio My notes (solo las notas).
export function Home({ onOpenNote, onNavigate, space }: { onOpenNote: (id: string) => void; onNavigate: (r: Route) => void; space?: boolean }) {
  const t = useT();
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [access, setAccess] = useState<CalendarAccess | null>(null);
  const [page, setPage] = useState(0);
  const [notes, setNotes] = useState<Note[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadEvents = useCallback(async () => {
    const a = await ipc.calendarAccess().catch(() => "unsupported" as const);
    setAccess(a);
    setEvents(a === "granted" ? await ipc.comingUp().catch(() => []) : []);
  }, []);

  useEffect(() => {
    if (space) return;
    loadEvents();
    // Cada minuto, al volver a la ventana (p. ej. desde Ajustes del Sistema)
    // y cuando se concede el permiso desde otra vista.
    const t = window.setInterval(loadEvents, 60_000);
    window.addEventListener("focus", loadEvents);
    const un = appEvents.calendarChanged(loadEvents);
    return () => {
      window.clearInterval(t);
      window.removeEventListener("focus", loadEvents);
      un.then((f) => f());
    };
  }, [loadEvents, space]);

  useEffect(() => {
    ipc.listNotes().then(setNotes).catch(() => setNotes([]));
  }, []);

  const connect = async () => {
    setError(null);
    const a = await ipc.requestCalendarAccess().catch((e) => {
      setError(String(e));
      return null;
    });
    if (a && a !== "granted") onNavigate({ kind: "settings", section: "calendar" });
    loadEvents();
  };
  const openEvent = async (e: CalendarEvent) => {
    try {
      const n = await ipc.openEventNote(e);
      onOpenNote(n.id);
    } catch (err) {
      setError(String(err));
    }
  };

  const pages = Math.max(1, Math.ceil(events.length / PAGE));
  const current = Math.min(page, pages - 1);

  return (
    <div className="home relative mx-auto flex min-h-full max-w-[860px] flex-col px-10">
      {space ? (
        <h1 className="page-title mb-8 mt-6 font-serif text-[34px] font-normal leading-tight">{t("nav.myNotes")}</h1>
      ) : (
        <>
          <div className="mb-5 mt-6 flex items-center">
            <h1 className="page-title flex-1 font-serif text-[34px] font-normal leading-tight">{t("home.comingUp")}</h1>
            <Button variant="ghost" size="iconSm" disabled={current === 0} onClick={() => setPage(current - 1)} aria-label={t("home.previous")}>
              <ChevronLeft />
            </Button>
            <Button variant="ghost" size="iconSm" disabled={current >= pages - 1} onClick={() => setPage(current + 1)} aria-label={t("home.next")}>
              <ChevronRight />
            </Button>
          </div>
          <ComingUp
            events={events.slice(current * PAGE, current * PAGE + PAGE)}
            access={access}
            onConnect={connect}
            onOpen={openEvent}
            onCalendarSettings={() => onNavigate({ kind: "settings", section: "calendar" })}
          />
          {error && <p className="mt-2 text-[13.5px] text-danger">{error}</p>}
        </>
      )}

      <div className={space ? "" : "mt-10"}>
        {notes && <NoteList notes={notes} onOpen={onOpenNote} empty={t("home.noNotes")} />}
      </div>

      <div className="sticky bottom-0 -mx-10 mt-auto bg-gradient-to-t from-canvas from-65% to-transparent px-10 pb-6 pt-12">
        <AskAnythingBar onAsk={(q) => onNavigate({ kind: "chat", q })} />
      </div>
    </div>
  );
}

const dayKey = (d: Date) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;

function DateBadge({ date }: { date: Date }) {
  const today = dayKey(date) === dayKey(new Date());
  return (
    <div className="flex w-[200px] shrink-0 items-start gap-4 px-5 py-4">
      <span className="font-serif text-[40px] leading-none">{date.getDate()}</span>
      <span className="pt-0.5 text-[13.5px] leading-tight">
        <span className="flex items-center gap-1.5">
          {date.toLocaleDateString(dateLocale(), { month: "long" })}
          {today && <span className="size-1.5 rounded-full bg-[#d4553f]" />}
        </span>
        <span className="text-muted">{date.toLocaleDateString(dateLocale(), { weekday: "short" })}</span>
      </span>
    </div>
  );
}

function ComingUp({
  events,
  access,
  onConnect,
  onOpen,
  onCalendarSettings,
}: {
  events: CalendarEvent[];
  access: CalendarAccess | null;
  onConnect: () => void;
  onOpen: (e: CalendarEvent) => void;
  onCalendarSettings: () => void;
}) {
  const t = useT();
  if (events.length === 0) {
    const connected = access === "granted";
    return (
      <div className="coming-up flex rounded-[22px] border border-line bg-card p-3">
        <DateBadge date={new Date()} />
        <div className="flex flex-1 flex-col items-center justify-center gap-1 rounded-2xl border border-dashed border-line px-6 py-9 text-center">
          <CalendarClock className="mb-2 size-7 text-ink-2" strokeWidth={1.5} />
          <p className="text-[15px] text-ink-2">{t("home.noEvents")}</p>
          {access === null ? null : connected ? (
            <p className="text-[13.5px] text-muted">{t("home.noMeetings30")}</p>
          ) : access === "unsupported" ? (
            <p className="text-[13.5px] text-muted">{t("home.calendarMacOnly")}</p>
          ) : (
            <>
              <p className="text-[13.5px] text-muted">{t("home.connectHint")}</p>
              <Button className="calendar-connect mt-4" onClick={access === "not_determined" ? onConnect : onCalendarSettings}>
                {access === "not_determined" ? t("home.connectCalendar") : t("home.calendarSettings")}
              </Button>
            </>
          )}
        </div>
      </div>
    );
  }

  // Un bloque por día: la fecha a la izquierda y sus eventos a la derecha.
  const days: [Date, CalendarEvent[]][] = [];
  for (const e of events) {
    const d = new Date(e.start);
    const last = days[days.length - 1];
    if (last && dayKey(last[0]) === dayKey(d)) last[1].push(e);
    else days.push([d, [e]]);
  }
  return (
    <div className="coming-up divide-y divide-line rounded-[22px] border border-line bg-card px-3">
      {days.map(([day, list]) => (
        <div key={dayKey(day)} className="flex py-3">
          <DateBadge date={day} />
          <ul className="flex-1">
            {list.map((e) => {
              const start = new Date(e.start);
              const live = start <= new Date();
              const others = e.attendees.filter((a) => !a.is_self).length;
              return (
                <li key={e.id}>
                  <button className="coming-event flex w-full gap-4 rounded-xl px-2 py-2 text-left hover:bg-hover" data-event-id={e.id} onClick={() => onOpen(e)}>
                    <span className="w-[3px] shrink-0 rounded-full bg-event" />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[15.5px]">{e.title || t("common.untitled")}</span>
                      <span className="flex items-center gap-3 text-[13px] text-muted">
                        <span>
                          {live ? t("date.now") : timeLabel(start)} – {timeLabel(new Date(e.end))}
                        </span>
                        {others > 0 && (
                          <span className="flex items-center gap-1">
                            <UsersRound className="size-3.5" /> {others}
                          </span>
                        )}
                        {e.conference_url && <Video className="size-3.5" aria-label={t("home.videoCall")} />}
                      </span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </div>
  );
}
