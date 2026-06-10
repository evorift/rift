<script lang="ts">
  // Onay diyaloğu (docs/02 §5-6). Riskli/yıkıcı aksiyonlarda kullanılır.
  import { fade, scale } from "svelte/transition";

  let {
    open = $bindable(false),
    title = "",
    message = "",
    confirmLabel = "Onayla",
    cancelLabel = "İptal",
    danger = false,
    hideCancel = false,
    onconfirm,
    oncancel,
  }: {
    open?: boolean;
    title?: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    danger?: boolean;
    hideCancel?: boolean;
    onconfirm?: () => void;
    oncancel?: () => void;
  } = $props();

  function cancel() {
    open = false;
    oncancel?.();
  }
  function confirm() {
    open = false;
    onconfirm?.();
  }
  function onkey(e: KeyboardEvent) {
    if (e.key === "Escape") cancel();
  }
</script>

<svelte:window onkeydown={open ? onkey : undefined} />

{#if open}
  <div class="overlay" transition:fade={{ duration: 120 }}>
    <button class="backdrop" aria-label="Kapat" onclick={cancel}></button>
    <div
      class="dialog"
      role="alertdialog"
      aria-modal="true"
      aria-label={title}
      transition:scale={{ duration: 140, start: 0.96 }}
    >
      {#if title}<h2 class="title" class:danger>{title}</h2>{/if}
      {#if message}<p class="msg">{message}</p>{/if}
      <div class="actions">
        {#if !hideCancel}<button class="btn" onclick={cancel}>{cancelLabel}</button>{/if}
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn {danger ? 'destructive' : 'primary'}" autofocus onclick={confirm}>
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 900;
    display: grid; place-items: center;
  }
  .backdrop {
    position: absolute; inset: 0; border: none; cursor: default;
    background: rgba(2, 4, 8, 0.6); backdrop-filter: blur(2px);
  }
  .dialog {
    position: relative; z-index: 1;
    width: min(420px, calc(100vw - 48px));
    background: var(--bg-surface); border: 1px solid var(--border);
    border-radius: var(--radius); padding: 22px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.55);
  }
  .title { font-size: 17px; font-weight: 700; margin-bottom: 8px; }
  .title.danger { color: var(--red); }
  .msg { color: var(--text-muted); line-height: 1.55; font-size: 14px; white-space: pre-line; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 20px; }
  .btn.destructive { background: var(--red); color: #fff; }
  .btn.destructive:hover { filter: brightness(1.08); }
</style>
