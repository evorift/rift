<script lang="ts">
  import { onMount } from "svelte";

  let { status = "off", onToggle }: { status?: "off" | "connecting" | "on"; onToggle?: () => void } = $props();

  const SIZE = 210;
  const CX = SIZE / 2, CY = SIZE / 2;
  const N = 90;

  type P = { x: number; y: number; vx: number; vy: number; r: number };
  let parts: P[] = [];
  let canvas: HTMLCanvasElement;
  let raf = 0;

  function build() {
    parts = [];
    for (let i = 0; i < N; i++) {
      parts.push({
        x: Math.random() * SIZE,
        y: Math.random() * SIZE,
        vx: (Math.random() - 0.5) * 0.6,
        vy: (Math.random() - 0.5) * 0.6,
        r: 1 + Math.random() * 1.6,
      });
    }
  }

  function explode() {
    for (const p of parts) {
      const dx = p.x - CX, dy = p.y - CY;
      const d = Math.hypot(dx, dy) || 1;
      const spd = 6 + Math.random() * 8;
      p.vx += (dx / d) * spd;
      p.vy += (dy / d) * spd;
    }
  }

  let prev = status;
  $effect(() => {
    if (status !== prev) {
      if (status === "connecting" || status === "on") explode();
      prev = status;
    }
  });

  function frame() {
    const ctx = canvas.getContext("2d")!;
    ctx.clearRect(0, 0, SIZE, SIZE);
    const color = status === "on" ? "34,211,238" : status === "connecting" ? "240,160,40" : "120,130,150";
    const alpha = status === "on" ? 0.8 : 0.45;

    for (const p of parts) {
      // serbest dolaşım: hafif sönüm + minik rastgele dürtü, kenarlardan sek
      p.vx = p.vx * 0.99 + (Math.random() - 0.5) * 0.05;
      p.vy = p.vy * 0.99 + (Math.random() - 0.5) * 0.05;
      p.x += p.vx; p.y += p.vy;
      if (p.x < p.r) { p.x = p.r; p.vx = Math.abs(p.vx) * 0.7; }
      if (p.x > SIZE - p.r) { p.x = SIZE - p.r; p.vx = -Math.abs(p.vx) * 0.7; }
      if (p.y < p.r) { p.y = p.r; p.vy = Math.abs(p.vy) * 0.7; }
      if (p.y > SIZE - p.r) { p.y = SIZE - p.r; p.vy = -Math.abs(p.vy) * 0.7; }
      // çok yavaşlarsa hafif canlandır
      const sp = Math.hypot(p.vx, p.vy);
      if (sp < 0.25) { p.vx += (Math.random() - 0.5) * 0.5; p.vy += (Math.random() - 0.5) * 0.5; }

      ctx.beginPath();
      ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(${color},${alpha})`;
      ctx.fill();
    }
    raf = requestAnimationFrame(frame);
  }

  onMount(() => {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = SIZE * dpr; canvas.height = SIZE * dpr;
    canvas.getContext("2d")!.scale(dpr, dpr);
    build();
    const onVis = () => {
      if (document.hidden) { cancelAnimationFrame(raf); raf = 0; }
      else if (!raf) raf = requestAnimationFrame(frame);
    };
    document.addEventListener("visibilitychange", onVis);
    raf = requestAnimationFrame(frame);
    return () => { cancelAnimationFrame(raf); document.removeEventListener("visibilitychange", onVis); };
  });
</script>

<button class="emblem {status}" style="width:{SIZE}px;height:{SIZE}px" onclick={onToggle} aria-label="Korumayı aç/kapat">
  <canvas bind:this={canvas} style="width:{SIZE}px;height:{SIZE}px"></canvas>

  <!-- ortadaki tek-parça "güvendesin" amblemi (parçacık değil) -->
  <span class="safe" class:show={status === "on"}>
    <svg viewBox="0 0 48 48">
      <path d="M24 4 L40 10 V23 C40 33 33 41 24 44 C15 41 8 33 8 23 V10 Z"
            fill="var(--accent)" />
      <path d="M16.5 24 l5 5 10-11" fill="none" stroke="#042027"
            stroke-width="3.4" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </span>
</button>

<style>
  .emblem {
    position: relative; border: none; background: transparent; cursor: pointer;
    display: grid; place-items: center; padding: 0;
    transition: transform .1s;
  }
  .emblem:active { transform: scale(.98); }
  canvas { display: block; }

  .safe {
    position: absolute; left: 50%; top: 50%;
    width: 92px; height: 92px;
    transform: translate(-50%, -50%) scale(.5);
    opacity: 0; pointer-events: none;
    transition: opacity .5s ease, transform .55s cubic-bezier(.2,.7,.2,1);
    filter: drop-shadow(0 0 16px var(--accent-glow));
  }
  .safe svg { width: 100%; height: 100%; display: block; }
  /* güvenli olunca: patlamadan SONRA belirsin (gecikme) */
  .emblem.on .safe.show { opacity: 1; transform: translate(-50%, -50%) scale(1); transition-delay: .45s; }
</style>
