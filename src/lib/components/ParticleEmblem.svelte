<script lang="ts">
  import { onMount } from "svelte";

  let { status = "off", onToggle }: { status?: "off" | "connecting" | "on"; onToggle?: () => void } = $props();

  const SIZE = 210;
  const CX = SIZE / 2, CY = SIZE / 2;
  const R = 66;
  const N = 110;

  type P = { x: number; y: number; vx: number; vy: number; ex: number; ey: number; sx: number; sy: number; r: number };
  let parts: P[] = [];
  let canvas: HTMLCanvasElement;
  let raf = 0;

  // ◈ amblem hedefleri: dış + iç elmas konturu
  function diamondPoint(t: number, rad: number) {
    // t: 0..1 elmasın çevresinde
    const verts = [
      [CX, CY - rad], [CX + rad, CY], [CX, CY + rad], [CX - rad, CY],
    ];
    const seg = t * 4;
    const i = Math.floor(seg) % 4;
    const f = seg - Math.floor(seg);
    const a = verts[i], b = verts[(i + 1) % 4];
    return [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f];
  }

  function build() {
    parts = [];
    for (let i = 0; i < N; i++) {
      const outer = i < N * 0.62;
      const t = (outer ? i / (N * 0.62) : (i - N * 0.62) / (N * 0.38));
      const [ex, ey] = diamondPoint(t, outer ? R : R * 0.46); // emblem (gizli) hedef
      const ang = Math.random() * Math.PI * 2;
      const sr = R * (1.05 + Math.random() * 0.55);          // scatter (açık) hedef
      const sx = CX + Math.cos(ang) * sr, sy = CY + Math.sin(ang) * sr;
      const start = status === "on" ? [ex, ey] : [sx, sy];
      parts.push({ x: start[0], y: start[1], vx: 0, vy: 0, ex, ey, sx, sy, r: 1.1 + Math.random() * 1.3 });
    }
  }

  function explode() {
    for (const p of parts) {
      const dx = p.x - CX, dy = p.y - CY;
      const d = Math.hypot(dx, dy) || 1;
      const spd = 7 + Math.random() * 7;
      p.vx += (dx / d) * spd;
      p.vy += (dy / d) * spd;
    }
  }

  let prev = status;
  $effect(() => {
    // duruma göre patlama tetikle
    if (status !== prev) {
      if (status === "connecting" || status === "on") explode();
      prev = status;
    }
  });

  function frame() {
    const ctx = canvas.getContext("2d")!;
    const k = 0.045, damp = 0.9;
    ctx.clearRect(0, 0, SIZE, SIZE);

    const hidden = status === "on";
    const accent = hidden ? "34,211,238" : status === "connecting" ? "240,160,40" : "120,130,150";

    for (const p of parts) {
      const tx = hidden ? p.ex : p.sx;
      const ty = hidden ? p.ey : p.sy;
      p.vx += (tx - p.x) * k; p.vy += (ty - p.y) * k;
      p.vx *= damp; p.vy *= damp;
      // hafif canlılık
      if (hidden) { p.vx += (Math.random() - 0.5) * 0.15; p.vy += (Math.random() - 0.5) * 0.15; }
      p.x += p.vx; p.y += p.vy;

      const speed = Math.min(1, Math.hypot(p.vx, p.vy) / 6);
      const alpha = hidden ? 0.9 : 0.5;
      ctx.beginPath();
      ctx.arc(p.x, p.y, p.r + speed * 1.5, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(${accent},${alpha})`;
      ctx.fill();
    }

    // gizliyken merkezde yumuşak parıltı
    if (hidden) {
      const g = ctx.createRadialGradient(CX, CY, 0, CX, CY, R * 0.7);
      g.addColorStop(0, "rgba(34,211,238,0.16)");
      g.addColorStop(1, "rgba(34,211,238,0)");
      ctx.fillStyle = g;
      ctx.fillRect(0, 0, SIZE, SIZE);
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
  <span class="hint">{status === "on" ? "GİZLİ" : status === "connecting" ? "…" : "AÇIK"}</span>
</button>

<style>
  .emblem {
    position: relative; border: none; background: transparent; cursor: pointer;
    display: grid; place-items: center; padding: 0;
    transition: transform .1s;
  }
  .emblem:active { transform: scale(.98); }
  canvas { display: block; }
  .hint {
    position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%);
    font-size: 12px; font-weight: 700; letter-spacing: 2px;
    color: var(--text-dim); pointer-events: none;
    transition: color .3s;
  }
  .emblem.on .hint { color: var(--accent); }
  .emblem.connecting .hint { color: var(--amber); }
</style>
