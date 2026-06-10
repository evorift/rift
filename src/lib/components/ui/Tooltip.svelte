<script lang="ts">
  // CSS-driven tooltip — hover/focus ile balon. Varsayılan tetikleyici küçük "?" ikonu.
  import type { Snippet } from "svelte";
  import RiskBadge from "./RiskBadge.svelte";
  let {
    text,
    level,
    children,
  }: {
    text: string;
    level?: "green" | "amber" | "red" | "break"; // verilirse balon içinde kesinti rozeti gösterilir
    children?: Snippet;
  } = $props();
</script>

<span class="tip">
  {#if children}
    {@render children()}
  {:else}
    <button class="q" aria-label="Bilgi" tabindex="0">?</button>
  {/if}
  <span class="bubble" role="tooltip">
    {#if level}<span class="risk"><RiskBadge {level} /></span>{/if}
    {text}
  </span>
</span>

<style>
  .tip { position: relative; display: inline-flex; align-items: center; }
  .q {
    width: 16px; height: 16px; border-radius: 50%; cursor: help;
    background: var(--bg-hover); border: 1px solid var(--border);
    color: var(--text-muted); font-size: 10px; font-weight: 700;
    display: grid; place-items: center; padding: 0;
  }
  .q:hover { color: var(--text); border-color: var(--accent-dim); }

  .bubble {
    position: absolute; bottom: calc(100% + 8px); left: 50%;
    transform: translateX(-50%) translateY(4px);
    width: max-content; max-width: 260px;
    padding: 8px 10px; border-radius: 8px;
    background: var(--bg-elevated); border: 1px solid var(--border);
    color: var(--text); font-size: 12px; font-weight: 500; line-height: 1.5;
    text-align: left; white-space: normal;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    opacity: 0; pointer-events: none;
    transition: opacity 0.14s ease, transform 0.14s ease;
    z-index: 50;
  }
  .tip:hover .bubble,
  .tip:focus-within .bubble {
    opacity: 1; transform: translateX(-50%) translateY(0);
  }
  .risk { display: block; margin-bottom: 6px; }
</style>
