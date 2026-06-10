<script lang="ts">
  // Stillendirilmiş native <select> dropdown (erişilebilir). Strateji/seçim menüleri için.
  type Opt = { value: string; label: string };
  let {
    value = $bindable(""),
    options,
    onchange,
    ariaLabel = "",
  }: {
    value?: string;
    options: Opt[];
    onchange?: (v: string) => void;
    ariaLabel?: string;
  } = $props();
</script>

<div class="dd">
  <select bind:value aria-label={ariaLabel} onchange={() => onchange?.(value)}>
    {#each options as o}
      <option value={o.value}>{o.label}</option>
    {/each}
  </select>
  <span class="caret" aria-hidden="true">▾</span>
</div>

<style>
  .dd { position: relative; display: inline-block; width: 100%; }
  select {
    appearance: none; -webkit-appearance: none;
    width: 100%; padding: 10px 34px 10px 12px;
    background: var(--bg-base); color: var(--text);
    border: 1px solid var(--border); border-radius: var(--radius-sm);
    font: inherit; font-size: 14px; font-weight: 600; cursor: pointer;
    transition: border-color 0.14s ease;
  }
  select:hover { border-color: var(--accent-dim); }
  select:focus { outline: none; border-color: var(--accent); }
  /* OS-render option listesi: koyu tema */
  option { background: var(--bg-elevated); color: var(--text); }
  .caret {
    position: absolute; right: 12px; top: 50%; transform: translateY(-50%);
    color: var(--text-muted); pointer-events: none; font-size: 12px;
  }
</style>
