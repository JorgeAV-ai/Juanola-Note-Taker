// Fechas al estilo de Granola: «Today», «Yesterday», «Fri 17 Jul»; horas en 24 h.
const dayKey = (d: Date) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;

export function dayLabel(d: Date, now = new Date()): string {
  if (dayKey(d) === dayKey(now)) return "Today";
  const y = new Date(now);
  y.setDate(now.getDate() - 1);
  if (dayKey(d) === dayKey(y)) return "Yesterday";
  const opts: Intl.DateTimeFormatOptions = { weekday: "short", day: "numeric", month: "short" };
  if (d.getFullYear() !== now.getFullYear()) opts.year = "numeric";
  return d.toLocaleDateString("en-GB", opts).replace(",", "");
}

export const timeLabel = (d: Date) => d.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });

export const shortDate = (d: Date) => d.toLocaleDateString("en-GB", { day: "numeric", month: "short" });

/** Agrupa por día manteniendo el orden de entrada. */
export function groupByDay<T>(items: T[], date: (t: T) => Date): [string, T[]][] {
  const out: [string, T[]][] = [];
  for (const it of items) {
    const label = dayLabel(date(it));
    const last = out[out.length - 1];
    if (last && last[0] === label) last[1].push(it);
    else out.push([label, [it]]);
  }
  return out;
}
