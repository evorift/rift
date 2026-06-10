// Hafif toast/snackbar store (docs/02 §5). Singleton; her yerden çağrılır.
export type ToastKind = "info" | "success" | "warn" | "error";
export type Toast = { id: number; kind: ToastKind; msg: string };

class Toasts {
  items = $state<Toast[]>([]);
  #id = 0;

  push(msg: string, kind: ToastKind = "info", ttl = 3200) {
    const id = ++this.#id;
    this.items = [...this.items, { id, kind, msg }];
    if (ttl > 0) setTimeout(() => this.dismiss(id), ttl);
    return id;
  }

  info(m: string) { return this.push(m, "info"); }
  success(m: string) { return this.push(m, "success"); }
  warn(m: string) { return this.push(m, "warn"); }
  error(m: string) { return this.push(m, "error"); }

  dismiss(id: number) {
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toasts = new Toasts();
