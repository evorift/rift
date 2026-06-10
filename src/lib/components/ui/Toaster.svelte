<script lang="ts">
  import { toasts } from "$lib/toast.svelte";
  import { fly } from "svelte/transition";

  const icon: Record<string, string> = { info: "ℹ", success: "✓", warn: "⚠", error: "✕" };
</script>

<div class="toaster" aria-live="polite" aria-atomic="false">
  {#each toasts.items as t (t.id)}
    <div
      class="toast t-{t.kind}"
      role="status"
      transition:fly={{ x: 24, duration: 180 }}
    >
      <span class="ic">{icon[t.kind]}</span>
      <span class="msg">{t.msg}</span>
      <button class="x" onclick={() => toasts.dismiss(t.id)} aria-label="Kapat">✕</button>
    </div>
  {/each}
</div>

<style>
  .toaster {
    position: fixed; right: 18px; bottom: 18px; z-index: 1000;
    display: flex; flex-direction: column; gap: 10px; pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex; align-items: center; gap: 10px;
    min-width: 240px; max-width: 360px;
    padding: 11px 12px 11px 13px; border-radius: var(--radius-sm);
    background: var(--bg-elevated); border: 1px solid var(--border);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
    font-size: 13.5px;
  }
  .ic {
    width: 20px; height: 20px; flex: 0 0 auto; border-radius: 50%;
    display: grid; place-items: center; font-size: 12px; font-weight: 700;
    color: var(--bg-base);
  }
  .msg { flex: 1 1 auto; line-height: 1.4; }
  .x {
    background: none; border: none; color: var(--text-dim); cursor: pointer;
    font-size: 12px; padding: 2px 4px; border-radius: 4px; transition: color 0.12s;
  }
  .x:hover { color: var(--text); }

  .t-info { border-left: 3px solid var(--accent); }
  .t-info .ic { background: var(--accent); }
  .t-success { border-left: 3px solid var(--green); }
  .t-success .ic { background: var(--green); }
  .t-warn { border-left: 3px solid var(--amber); }
  .t-warn .ic { background: var(--amber); }
  .t-error { border-left: 3px solid var(--red); }
  .t-error .ic { background: var(--red); color: #fff; }
</style>
