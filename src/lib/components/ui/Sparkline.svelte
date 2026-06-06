<script lang="ts">
  // Canvas sparkline — tek canvas, per-point SVG yok (docs/02 §3-4).
  let {
    data = [],
    color = "var(--accent)",
    height = 56,
    fill = true,
  }: {
    data?: number[];
    color?: string;
    height?: number;
    fill?: boolean;
  } = $props();

  let canvas: HTMLCanvasElement | undefined = $state();

  function resolveColor(c: string): string {
    if (!c.startsWith("var(")) return c;
    const name = c.slice(4, -1).trim();
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#22D3EE";
  }

  function draw() {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
      canvas.width = w * dpr; canvas.height = h * dpr;
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    if (data.length < 2) return;
    const min = Math.min(...data);
    const max = Math.max(...data);
    const range = max - min || 1;
    const pad = 3;
    const stepX = w / (data.length - 1);
    const y = (v: number) => pad + (1 - (v - min) / range) * (h - pad * 2);

    const stroke = resolveColor(color);

    ctx.beginPath();
    data.forEach((v, i) => {
      const px = i * stepX;
      const py = y(v);
      i === 0 ? ctx.moveTo(px, py) : ctx.lineTo(px, py);
    });

    if (fill) {
      const grad = ctx.createLinearGradient(0, 0, 0, h);
      grad.addColorStop(0, stroke + "44");
      grad.addColorStop(1, stroke + "00");
      ctx.save();
      ctx.lineTo(w, h); ctx.lineTo(0, h); ctx.closePath();
      ctx.fillStyle = grad;
      ctx.fill();
      ctx.restore();
      // çizgiyi yeniden çiz (fill closePath'i bozdu)
      ctx.beginPath();
      data.forEach((v, i) => {
        const px = i * stepX;
        const py = y(v);
        i === 0 ? ctx.moveTo(px, py) : ctx.lineTo(px, py);
      });
    }

    ctx.strokeStyle = stroke;
    ctx.lineWidth = 1.75;
    ctx.lineJoin = "round";
    ctx.lineCap = "round";
    ctx.stroke();
  }

  // data değişince yeniden çiz (1 Hz tick — ucuz)
  $effect(() => {
    void data;
    draw();
  });
</script>

<canvas bind:this={canvas} style="height: {height}px"></canvas>

<style>
  canvas { width: 100%; display: block; }
</style>
