import { invoke } from "@tauri-apps/api/core";

type Status = "off" | "connecting" | "on";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export const HISTORY_LEN = 80; // sparkline örnek sayısı

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

class AppState {
  status = $state<Status>("off");

  // ---- canlı metrikler (şimdilik simüle; servis IPC gelince oradan beslenecek) ----
  ping = $state(0); // ms
  jitter = $state(0); // ms
  loss = $state(0); // %
  down = $state(0); // Mbps
  up = $state(0); // Mbps
  pingHistory = $state<number[]>([]);

  // ---- bağlantı / strateji seçimleri (bölümler arası kalıcı) ----
  strategy = $state<string>("auto");
  dns = $state<string>("cloudflare");

  // ---- ayarlar ----
  autostart = $state(false);
  startMinimized = $state(false);
  reduceMotion = $state(false);
  language = $state<"tr" | "en" | "es" | "ru">("tr");

  #timer: ReturnType<typeof setInterval> | null = null;
  #onVis: (() => void) | null = null;

  async init() {
    try {
      const s = await invoke<{ running: boolean }>("protection_status");
      this.status = s.running ? "on" : "off";
    } catch (_) {}
    if (this.status === "on") this.#startMetrics();

    // pencere gizli/blur olunca simülasyonu durdur — en büyük idle tasarrufu (bkz. docs/02 §3)
    if (typeof document !== "undefined") {
      this.#onVis = () => {
        if (document.hidden) this.#stopMetrics();
        else if (this.status === "on") this.#startMetrics();
      };
      document.addEventListener("visibilitychange", this.#onVis);
    }
  }

  async toggle() {
    if (this.status === "connecting") return;
    if (this.status === "on") {
      try { await invoke("stop_protection"); } catch (_) {}
      this.status = "off";
      this.#stopMetrics();
      this.#resetMetrics();
      return;
    }
    this.status = "connecting";
    try {
      await Promise.all([invoke("start_protection"), sleep(800)]);
      this.status = "on";
      this.#startMetrics();
    } catch (_) {
      this.status = "off";
    }
  }

  // ---- metrik simülatörü ----
  #startMetrics() {
    if (this.#timer || typeof document === "undefined" || document.hidden) return;
    if (this.pingHistory.length === 0) this.#seedHistory();
    this.#timer = setInterval(() => this.#tick(), 1000); // 1 Hz polling (docs/02 §4)
  }

  #stopMetrics() {
    if (this.#timer) { clearInterval(this.#timer); this.#timer = null; }
  }

  #resetMetrics() {
    this.ping = 0; this.jitter = 0; this.loss = 0; this.down = 0; this.up = 0;
    this.pingHistory = [];
  }

  #seedHistory() {
    const base = 24;
    this.pingHistory = Array.from({ length: HISTORY_LEN }, () =>
      Math.round(base + (Math.random() - 0.5) * 6)
    );
    this.ping = this.pingHistory[this.pingHistory.length - 1];
  }

  #tick() {
    // korumalı: düşük & stabil ping; küçük dalgalanma
    const prev = this.ping || 24;
    const next = clamp(Math.round(prev + (Math.random() - 0.5) * 5), 14, 42);
    this.ping = next;
    this.jitter = clamp(Math.round(Math.abs(next - prev) + Math.random() * 2), 0, 12);
    this.loss = Math.random() < 0.85 ? 0 : +(Math.random() * 1.2).toFixed(1);
    this.down = +(Math.random() * 2.4).toFixed(1);
    this.up = +(Math.random() * 0.9).toFixed(1);

    const h = this.pingHistory.slice(-(HISTORY_LEN - 1));
    h.push(next);
    this.pingHistory = h;
  }
}

export const app = new AppState();
