import { invoke } from "@tauri-apps/api/core";

type Status = "off" | "connecting" | "on";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

class AppState {
  status = $state<Status>("off");

  async init() {
    try {
      const s = await invoke<{ running: boolean }>("protection_status");
      this.status = s.running ? "on" : "off";
    } catch (_) {}
  }

  async toggle() {
    if (this.status === "connecting") return;
    if (this.status === "on") {
      try { await invoke("stop_protection"); } catch (_) {}
      this.status = "off";
      return;
    }
    this.status = "connecting";
    try {
      await Promise.all([invoke("start_protection"), sleep(800)]);
      this.status = "on";
    } catch (_) {
      this.status = "off";
    }
  }
}

export const app = new AppState();
