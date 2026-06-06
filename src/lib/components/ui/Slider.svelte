<script lang="ts">
  // Etiketli + canlı değer balonlu slider. Native range; balon transform ile konumlanır.
  let {
    value = $bindable(0),
    min = 0,
    max = 100,
    step = 1,
    label = "",
    unit = "",
    onchange,
  }: {
    value?: number;
    min?: number;
    max?: number;
    step?: number;
    label?: string;
    unit?: string;
    onchange?: (v: number) => void;
  } = $props();

  const pct = $derived(((value - min) / (max - min)) * 100);
</script>

<div class="slider">
  {#if label}
    <div class="row">
      <span class="lbl">{label}</span>
      <span class="val mono">{value}{unit}</span>
    </div>
  {/if}
  <div class="track-wrap" style="--pct: {pct}%">
    <input
      type="range"
      {min}
      {max}
      {step}
      bind:value
      oninput={() => onchange?.(value)}
      aria-label={label}
    />
    <span class="fill" aria-hidden="true"></span>
  </div>
</div>

<style>
  .slider { display: flex; flex-direction: column; gap: 8px; }
  .row { display: flex; align-items: center; justify-content: space-between; }
  .lbl { font-size: 13px; font-weight: 600; color: var(--text-muted); }
  .val { font-size: 13px; font-weight: 700; color: var(--accent); }

  .track-wrap { position: relative; height: 18px; display: flex; align-items: center; }
  .fill {
    position: absolute; left: 0; top: 50%; height: 4px; width: var(--pct);
    transform: translateY(-50%); border-radius: 999px;
    background: var(--accent); pointer-events: none;
  }
  input[type="range"] {
    -webkit-appearance: none; appearance: none; width: 100%; height: 4px;
    background: var(--bg-hover); border-radius: 999px; outline: none; margin: 0;
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none; appearance: none;
    width: 14px; height: 14px; border-radius: 50%;
    background: var(--text); border: 2px solid var(--accent);
    cursor: pointer; position: relative; z-index: 2;
    transition: transform 0.1s ease;
  }
  input[type="range"]:active::-webkit-slider-thumb { transform: scale(1.15); }
</style>
