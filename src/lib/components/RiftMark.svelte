<script lang="ts">
  import { app } from "$lib/state.svelte";

  const on = $derived(app.status === "on");
  const connecting = $derived(app.status === "connecting");

  let el: HTMLButtonElement;
  let rx = $state(0), ry = $state(0);
  function move(e: PointerEvent) {
    const r = el.getBoundingClientRect();
    rx = -(((e.clientY - r.top) / r.height) - 0.5) * 22;
    ry = (((e.clientX - r.left) / r.width) - 0.5) * 22;
  }
  function reset() { rx = 0; ry = 0; }
</script>

<button
  class="mark"
  class:on
  class:connecting
  bind:this={el}
  onclick={() => app.toggle()}
  onpointermove={move}
  onpointerleave={reset}
  style="transform: perspective(720px) rotateX({rx}deg) rotateY({ry}deg);"
  aria-label="Rift · aç/kapat"
>
  <svg viewBox="0 0 100 100">
    <defs>
      <mask id="riftcut">
        <rect width="100" height="100" fill="white" />
        <!-- çapraz yarık (rift) -->
        <rect x="-25" y="45.5" width="150" height="9" fill="black" transform="rotate(-32 50 50)" />
      </mask>
    </defs>
    <circle cx="50" cy="50" r="44" mask="url(#riftcut)" />
    <!-- yarık boyunca akan parıltı -->
    <line class="spark" x1="16" y1="71" x2="84" y2="29" />
  </svg>
</button>

<style>
  .mark {
    width: 100%; height: 100%; border: none; background: transparent; padding: 0;
    cursor: pointer; display: grid; place-items: center;
    transition: transform .12s ease-out;
    transform-style: preserve-3d;
  }
  .mark svg { width: 100%; height: 100%; overflow: visible; }

  /* gövde rengi: kapalı sönük, açık beyaz */
  .mark circle { fill: #3a4150; transition: fill .35s ease, filter .35s ease; }
  .mark.connecting circle { fill: #c7ccd6; animation: pulse 1s ease-in-out infinite; }
  .mark.on circle { fill: #ffffff; filter: drop-shadow(0 0 10px rgba(255,255,255,.35)); }

  /* yarık parıltısı (hareket) — yalnız açıkken */
  .spark { stroke: transparent; stroke-width: 2.4; stroke-linecap: round; }
  .mark.on .spark {
    stroke: rgba(255,255,255,.9);
    stroke-dasharray: 14 86;
    animation: sweep 1.8s linear infinite;
  }

  @keyframes sweep { from { stroke-dashoffset: 100; } to { stroke-dashoffset: 0; } }
  @keyframes pulse { 0%,100% { opacity: .7; } 50% { opacity: 1; } }

  @media (prefers-reduced-motion: reduce) {
    .mark.on .spark { animation: none; }
    .mark.connecting circle { animation: none; }
  }
</style>
