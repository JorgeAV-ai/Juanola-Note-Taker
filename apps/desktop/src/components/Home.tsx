import { useEffect, useState } from "react";
import { CalendarClock, ChevronLeft, ChevronRight } from "lucide-react";
import type { Route } from "@/App";
import { ipc, type CalendarEvent, type Note } from "@/lib/ipc";
import { Button } from "@/components/ui/button";
import { NoteList } from "@/components/NoteList";
import { AskAnythingBar } from "@/components/AskAnythingBar";

// Home de Granola: «Coming up» con el calendario y las notas por día. Con
// `space`, la vista del espacio My notes (solo las notas).
export function Home({ onOpenNote, onNavigate, space }: { onOpenNote: (id: string) => void; onNavigate: (r: Route) => void; space?: boolean }) {
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [notes, setNotes] = useState<Note[] | null>(null);

  useEffect(() => {
    ipc.comingUp().then(setEvents).catch(() => setEvents([]));
    ipc.listNotes().then(setNotes).catch(() => setNotes([]));
  }, []);

  return (
    <div className="home relative mx-auto flex min-h-full max-w-[860px] flex-col px-10">
      {space ? (
        <h1 className="page-title mb-8 mt-6 font-serif text-[34px] font-normal leading-tight">My notes</h1>
      ) : (
        <>
          <div className="mb-5 mt-6 flex items-center">
            <h1 className="page-title flex-1 font-serif text-[34px] font-normal leading-tight">Coming up</h1>
            <Button variant="ghost" size="iconSm" disabled aria-label="Anterior">
              <ChevronLeft />
            </Button>
            <Button variant="ghost" size="iconSm" disabled aria-label="Siguiente">
              <ChevronRight />
            </Button>
          </div>
          <ComingUp events={events} onCalendarSettings={() => onNavigate({ kind: "settings", section: "calendar" })} />
        </>
      )}

      <div className={space ? "" : "mt-10"}>
        {notes && <NoteList notes={notes} onOpen={onOpenNote} empty="Todavía no hay notas." />}
      </div>

      <div className="sticky bottom-0 -mx-10 mt-auto bg-gradient-to-t from-canvas from-65% to-transparent px-10 pb-6 pt-12">
        <AskAnythingBar onAsk={(q) => onNavigate({ kind: "chat", q })} />
      </div>
    </div>
  );
}

function ComingUp({ events, onCalendarSettings }: { events: CalendarEvent[]; onCalendarSettings: () => void }) {
  const today = new Date();
  return (
    <div className="coming-up flex rounded-[22px] border border-line bg-card p-3">
      <div className="flex w-[200px] shrink-0 items-start gap-4 px-5 py-4">
        <span className="font-serif text-[40px] leading-none">{today.getDate()}</span>
        <span className="pt-0.5 text-[13.5px] leading-tight">
          <span className="flex items-center gap-1.5">
            {today.toLocaleDateString("en-GB", { month: "long" })}
            <span className="size-1.5 rounded-full bg-[#d4553f]" />
          </span>
          <span className="text-muted">{today.toLocaleDateString("en-GB", { weekday: "short" })}</span>
        </span>
      </div>
      {events.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-1 rounded-2xl border border-dashed border-line px-6 py-9 text-center">
          <CalendarClock className="mb-2 size-7 text-ink-2" strokeWidth={1.5} />
          <p className="text-[15px] text-ink-2">No upcoming events</p>
          <p className="text-[13.5px] text-muted">La integración con el calendario aún no está hecha</p>
          <Button className="mt-4" onClick={onCalendarSettings}>
            Calendar settings
          </Button>
        </div>
      ) : (
        <ul className="flex-1 py-3">
          {events.slice(0, 5).map((e) => (
            <li key={e.id} className="flex gap-4 py-2">
              <span className="w-[3px] rounded-full bg-event" />
              <span>
                <span className="block text-[15.5px]">{e.title}</span>
                <span className="text-[13px] text-muted">{new Date(e.start).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" })}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
