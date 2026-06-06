<script lang="ts">
  import { onMount } from "svelte";

  let { status = "off", onToggle }: { status?: "off" | "connecting" | "on"; onToggle?: () => void } = $props();

  const SIZE = 230;
  const CX = SIZE / 2, CY = SIZE / 2;

  // mavi-mor palet
  const BLUE = "#3C6EFF", PURPLE = "#9646FF", BRIGHT = "#bcd0ff";

  type P = { x: number; y: number; px: number; py: number; vx: number; vy: number; r: number; dead: boolean };
  let parts: P[] = [];
  let canvas: HTMLCanvasElement;
  let raf = 0;

  // faz makinesi: closed -> burst -> suck -> open
  let phase: "closed" | "burst" | "suck" | "open" = "closed";
  let openStart = 0;
  let timers: number[] = [];
  const clearTimers = () => { timers.forEach(clearTimeout); timers = []; };

  function spawn() {
    parts = [];
    for (let i = 0; i < 80; i++) {
      const a = Math.random() * Math.PI * 2;
      const s = 5 + Math.random() * 9;
      parts.push({ x: CX, y: CY, px: CX, py: CY, vx: Math.cos(a) * s, vy: Math.sin(a) * s, r: 1 + Math.random() * 1.8, dead: false });
    }
  }

  function startOpen() {
    clearTimers();
    phase = "burst"; spawn();
    timers.push(setTimeout(() => { phase = "suck"; }, 520) as unknown as number);
    timers.push(setTimeout(() => { phase = "open"; openStart = performance.now(); }, 1200) as unknown as number);
  }
  function startClose() {
    clearTimers();
    phase = "closed"; parts = [];
  }

  let prev = status;
  $effect(() => {
    if (status !== prev) {
      if ((status === "connecting" || status === "on") && (prev === "off")) startOpen();
      else if (status === "off") startClose();
      prev = status;
    }
  });

  function drawClosed(ctx: CanvasRenderingContext2D) {
    ctx.save();
    ctx.translate(CX, CY);
    ctx.globalAlpha = 0.55;
    ctx.shadowColor = PURPLE; ctx.shadowBlur = 16;
    ctx.strokeStyle = "#6b6fae"; ctx.lineCap = "round"; ctx.lineWidth = 3;
    // kapalı rift: dikey yarık
    ctx.beginPath(); ctx.moveTo(0, -44); ctx.lineTo(0, 44); ctx.stroke();
    // uç parıltıları
    ctx.shadowBlur = 0; ctx.globalAlpha = 0.3; ctx.lineWidth = 1.2;
    ctx.beginPath(); ctx.moveTo(-10, 0); ctx.lineTo(10, 0); ctx.stroke();
    ctx.restore();
  }

  function drawPortal(ctx: CanvasRenderingContext2D, t: number) {
    const op = Math.min(1, (t - openStart) / 650);
    const ease = 1 - Math.pow(1 - op, 3);
    const rr = 56 * ease, rc = 30 * ease;
    const rot = t / 1000;

    ctx.save();
    ctx.translate(CX, CY);

    // dış glow
    const g = ctx.createRadialGradient(0, 0, rc, 0, 0, rr + 34);
    g.addColorStop(0, "rgba(150,70,255,0.35)");
    g.addColorStop(1, "rgba(60,110,255,0)");
    ctx.globalAlpha = ease;
    ctx.fillStyle = g;
    ctx.beginPath(); ctx.arc(0, 0, rr + 34, 0, Math.PI * 2); ctx.fill();

    // dönen halka (konik gradyan) — hafif elips (portal eğimi)
    ctx.save();
    ctx.scale(1, 0.9);
    const cg = ctx.createConicGradient(rot, 0, 0);
    cg.addColorStop(0, BLUE); cg.addColorStop(0.25, PURPLE);
    cg.addColorStop(0.5, BLUE); cg.addColorStop(0.7, BRIGHT);
    cg.addColorStop(0.85, PURPLE); cg.addColorStop(1, BLUE);
    ctx.fillStyle = cg;
    ctx.beginPath(); ctx.arc(0, 0, rr, 0, Math.PI * 2); ctx.fill();
    // çekirdeği oy → halka kalsın
    ctx.globalCompositeOperation = "destination-out";
    ctx.beginPath(); ctx.arc(0, 0, rc, 0, Math.PI * 2); ctx.fill();
    ctx.globalCompositeOperation = "source-over";
    ctx.restore();

    // karanlık çekirdek (olay ufku)
    const core = ctx.createRadialGradient(0, 0, 0, 0, 0, rc + 4);
    core.addColorStop(0, "#04050b");
    core.addColorStop(0.72, "#05060d");
    core.addColorStop(1, "rgba(5,6,13,0)");
    ctx.fillStyle = core;
    ctx.beginPath(); ctx.arc(0, 0, rc + 4, 0, Math.PI * 2); ctx.fill();

    // ışık-bükme parlak iç kenar
    ctx.shadowColor = BRIGHT; ctx.shadowBlur = 14;
    ctx.strokeStyle = "rgba(190,208,255,0.9)"; ctx.lineWidth = 1.6;
    ctx.beginPath(); ctx.ellipse(0, 0, rc + 1, (rc + 1) * 0.9, 0, 0, Math.PI * 2); ctx.stroke();

    ctx.restore();
  }

  function frame() {
    const ctx = canvas.getContext("2d")!;
    const t = performance.now();
    ctx.clearRect(0, 0, SIZE, SIZE);

    if (phase === "closed") { drawClosed(ctx); raf = requestAnimationFrame(frame); return; }
    if (phase === "open") drawPortal(ctx, t);

    // parçacıklar (burst / suck)
    if (phase === "burst" || phase === "suck") {
      for (const p of parts) {
        if (p.dead) continue;
        p.px = p.x; p.py = p.y;
        if (phase === "burst") {
          p.vx *= 0.95; p.vy *= 0.95;
        } else {
          // kara delik: merkeze hızlanan çekim + sündürme
          const dx = CX - p.x, dy = CY - p.y;
          const d = Math.hypot(dx, dy) || 1;
          const pull = 0.9 + (90 - Math.min(90, d)) * 0.02;
          p.vx = p.vx * 0.86 + (dx / d) * pull;
          p.vy = p.vy * 0.86 + (dy / d) * pull;
          if (d < 7) { p.dead = true; }
        }
        p.x += p.vx; p.y += p.vy;

        // çekimde hız yönünde sündür (motion-blur çizgi)
        const stretch = phase === "suck" ? 3.2 : 1.4;
        ctx.strokeStyle = "rgba(160,140,255,0.85)";
        ctx.lineWidth = p.r;
        ctx.lineCap = "round";
        ctx.beginPath();
        ctx.moveTo(p.x - p.vx * stretch, p.y - p.vy * stretch);
        ctx.lineTo(p.x, p.y);
        ctx.stroke();
      }
    }
    raf = requestAnimationFrame(frame);
  }

  onMount(() => {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = SIZE * dpr; canvas.height = SIZE * dpr;
    canvas.getContext("2d")!.scale(dpr, dpr);
    if (status === "on") { phase = "open"; openStart = performance.now(); }
    const onVis = () => {
      if (document.hidden) { cancelAnimationFrame(raf); raf = 0; }
      else if (!raf) raf = requestAnimationFrame(frame);
    };
    document.addEventListener("visibilitychange", onVis);
    raf = requestAnimationFrame(frame);
    return () => { cancelAnimationFrame(raf); clearTimers(); document.removeEventListener("visibilitychange", onVis); };
  });
</script>

<button class="emblem" style="width:{SIZE}px;height:{SIZE}px" onclick={onToggle} aria-label="Korumayı aç/kapat">
  <canvas bind:this={canvas} style="width:{SIZE}px;height:{SIZE}px"></canvas>
</button>

<style>
  .emblem {
    position: relative; border: none; background: transparent; cursor: pointer;
    display: grid; place-items: center; padding: 0;
    transition: transform .1s;
  }
  .emblem:active { transform: scale(.98); }
  canvas { display: block; }
</style>
