// Gerçek uygulama olay günlüğü (docs/03 §10 Health Check). Singleton; gerçek olayları kaydeder.
// Not: mesajlar Türkçe (teknik tanılama günlüğü, TR-öncelikli uygulama). Servis IPC bağlanınca
// canlı motor olaylarıyla da beslenecek. Önceki sürümdeki SAHTE statik satırların yerini aldı.
export type LogLevel = "info" | "ok" | "warn" | "error";
export type LogLine = { id: number; t: string; level: LogLevel; msg: string };

const MAX = 200;
function stamp(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

class EventLog {
  lines = $state<LogLine[]>([]);
  #id = 0;
  push(msg: string, level: LogLevel = "info") {
    const line: LogLine = { id: ++this.#id, t: stamp(), level, msg };
    const next = [...this.lines, line];
    this.lines = next.length > MAX ? next.slice(next.length - MAX) : next;
  }
  info(m: string) { this.push(m, "info"); }
  ok(m: string) { this.push(m, "ok"); }
  warn(m: string) { this.push(m, "warn"); }
  error(m: string) { this.push(m, "error"); }
  clear() { this.lines = []; }
}

export const eventLog = new EventLog();
