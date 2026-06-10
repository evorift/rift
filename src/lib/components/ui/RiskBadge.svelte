<script lang="ts">
  // Kesinti rozeti (docs/04 §"Kesinti tipi"). 4 seviye.
  import { t } from "$lib/i18n.svelte";
  export type RiskLevel = "green" | "amber" | "red" | "break";
  let { level, compact = false }: { level: RiskLevel; compact?: boolean } = $props();

  const dots: Record<RiskLevel, string> = { green: "🟢", amber: "🟡", red: "🔴", break: "⚠️" };
  const label = $derived(t("risk." + level));
</script>

<span class="badge b-{level}" title={label}>
  <span class="dot">{dots[level]}</span>
  {#if !compact}<span class="lbl">{label}</span>{/if}
</span>

<style>
  .badge {
    display: inline-flex; align-items: center; gap: 5px;
    padding: 2px 8px 2px 6px; border-radius: 999px;
    font-size: 11px; font-weight: 600; white-space: nowrap;
    background: var(--bg-elevated); border: 1px solid var(--border);
  }
  .dot { font-size: 9px; line-height: 1; }
  .b-green { color: var(--green); }
  .b-amber { color: var(--amber); }
  .b-red { color: var(--red); }
  .b-break { color: var(--red); border-color: color-mix(in srgb, var(--red) 55%, transparent); }
</style>
