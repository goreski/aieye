/** A timestamp from AIEye in the user's local time, e.g. "26 Oct 2026, 17:30". */
export function localTime(timestamp: string): string {
  // AIEye writes up to nanoseconds; JavaScript dates read milliseconds.
  const date = new Date(timestamp.replace(/(\.\d{3})\d+/, "$1"));
  return date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** "1 alert", "3 alerts". */
export function count(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}
