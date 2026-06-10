<script lang="ts">
  // Matrix-tarzı 01 binary yağmuru. Koruma/oyun modu açılırken (app.activating) düşer,
  // aktif olunca yumuşakça kaybolur. Tek canvas, compositor-dostu (docs/02 §3-4).
  import { app } from "$lib/state.svelte";
  import { onMount } from "svelte";

  let canvas: HTMLCanvasElement | undefined = $state();
  let ctx: CanvasRenderingContext2D | null = null;
  let raf = 0;
  let opacity = 0;
  let drops: number[] = [];
  let w = 0, h = 0;
  const fontSize = 16;

  const active = $derived(app.activating);
  const reduced = () =>
    app.reduceMotion ||
    (typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches);

  function accent() {
    return getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() || "#39E66B";
  }

  function resize() {
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    w = window.innerWidth; h = window.innerHeight;
    canvas.width = w * dpr; canvas.height = h * dpr;
    ctx = canvas.getContext("2d");
    ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
    const cols = Math.ceil(w / fontSize);
    drops = Array.from({ length: cols }, () => Math.floor((Math.random() * -h) / fontSize));
  }

  let tick = 0;
  // overlay olarak içeriğin üstünde göster → tüm ekranda görünür (max ~0.9 saydamlık)
  const MAX_OPACITY = 0.9;
  function frame() {
    if (!ctx || !canvas) { raf = 0; return; }
    const target = active ? MAX_OPACITY : 0;
    opacity += (target - opacity) * 0.08;

    // iz bırakma (siyah üstüne hafif) → klasik matrix kuyruğu
    ctx.fillStyle = "rgba(0,0,0,0.12)";
    ctx.fillRect(0, 0, w, h);

    ctx.font = `${fontSize}px "Cascadia Code", "Consolas", monospace`;
    const col = accent();
    tick++;
    for (let i = 0; i < drops.length; i++) {
      const x = i * fontSize;
      const y = drops[i] * fontSize;
      // baştaki damla parlak beyaz-yeşil, kuyruk yeşil
      ctx.fillStyle = Math.random() < 0.08 ? "#CFFFE0" : col;
      ctx.fillText(Math.random() < 0.5 ? "0" : "1", x, y);
      if (tick % 2 === 0) {
        if (y > h && Math.random() > 0.975) drops[i] = 0;
        else drops[i]++;
      }
    }

    canvas.style.opacity = String(Math.max(0, opacity));

    if (active || opacity > 0.02) {
      raf = requestAnimationFrame(frame);
    } else {
      opacity = 0;
      canvas.style.opacity = "0";
      ctx.clearRect(0, 0, w, h);
      raf = 0;
    }
  }

  // aktivasyon başlayınca döngüyü başlat (reduced-motion'da atla)
  $effect(() => {
    if (active && !raf && !reduced()) {
      resize();
      raf = requestAnimationFrame(frame);
    }
  });

  onMount(() => {
    const onResize = () => { if (raf) resize(); };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
      if (raf) cancelAnimationFrame(raf);
    };
  });
</script>

<canvas bind:this={canvas} class="rain" aria-hidden="true"></canvas>

<style>
  .rain {
    position: fixed; inset: 0; z-index: 90; pointer-events: none;
    width: 100vw; height: 100vh; opacity: 0;
  }
</style>
