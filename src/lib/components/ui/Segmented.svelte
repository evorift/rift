<script lang="ts">
  // Segmented control — strateji/profil seçici. Aktif vurgu pill'i transform ile kayar.
  type Option = { value: string; label: string };
  let {
    options,
    value = $bindable(""),
    onchange,
  }: {
    options: Option[];
    value?: string;
    onchange?: (v: string) => void;
  } = $props();

  const index = $derived(Math.max(0, options.findIndex((o) => o.value === value)));

  function pick(v: string) {
    value = v;
    onchange?.(v);
  }
</script>

<div class="seg" style="--n: {options.length}; --i: {index};" role="tablist">
  <span class="pill" aria-hidden="true"></span>
  {#each options as o}
    <button
      type="button"
      role="tab"
      class="opt"
      class:active={o.value === value}
      aria-selected={o.value === value}
      onclick={() => pick(o.value)}
    >{o.label}</button>
  {/each}
</div>

<style>
  .seg {
    position: relative; display: grid;
    grid-template-columns: repeat(var(--n), 1fr);
    gap: 2px; padding: 3px; border-radius: var(--radius-sm);
    background: var(--bg-base); border: 1px solid var(--border);
  }
  .pill {
    position: absolute; top: 3px; bottom: 3px; left: 3px;
    width: calc((100% - 6px) / var(--n));
    border-radius: calc(var(--radius-sm) - 2px);
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    transform: translateX(calc(var(--i) * 100%));
    transition: transform 0.2s cubic-bezier(.2,.8,.3,1);
    will-change: transform;
  }
  .opt {
    position: relative; z-index: 1; background: none; border: none;
    padding: 8px 10px; border-radius: calc(var(--radius-sm) - 2px);
    color: var(--text-muted); font: inherit; font-weight: 600; font-size: 13px;
    cursor: pointer; transition: color 0.16s ease; white-space: nowrap;
  }
  .opt:hover { color: var(--text); }
  .opt.active { color: var(--text); }
</style>
