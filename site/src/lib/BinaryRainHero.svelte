<script>
  // Uygulamadaki src/lib/components/BinaryRain.svelte'in doğrudan portu — kullanıcı
  // isteği: "binary rain uygulamadaki sveltten kopyala". Aynı tuning sabitleri, aynı
  // algoritma (yoğunluk SPAWN konumundan değil FADE MESAFESİNDEN geliyor: merkezdeki
  // damla ekranın çeyreğinde söner, kenardaki tam boy gider — bu, dikey şeritler yerine
  // "derinlik" gibi okunuyor). Farklar, yalnız site bağlamının gerektirdiği kadar:
  //   - app.state yok: site burada koşulsuz her zaman açık (uygulamada yalnız koruma
  //     GERÇEKTEN açıkken görünür; sitede öyle bir durum kavramı yok).
  //   - Tek renk: yeşil. Mor yalnız VPN/WARP bölümünde (marka kuralı, skill §4).
  //   - position:fixed, tüm viewport — kullanıcı kararı: üst bar'ın (topbar) ARKASINDAN
  //     da görünsün. Topbar zaten yarı saydam + blur'lu; yağmur onun altından sızıyor,
  //     üstündeki nav metni okunur kalıyor (topbar z-index:10, bundan yüksek). Bunun
  //     bedeli: yağmur artık FAQ/footer gibi hero dışı bölümlerde de görünür — sabit
  //     bir viewport katmanı, sayfayla birlikte kaymıyor.
  //   - prefers-reduced-motion: hiç canvas mount edilmez.
  //   - Dar viewport: yoğunluk düşürülür (kapatılmaz) — kara delik artık genişlik
  //     yüzünden hiç durmuyor, aynı ilkeyi yağmura da uyguluyoruz.
  import { onMount } from "svelte";

  let canvas;
  let animId = 0;
  let visible = $state(false);

  const CHAR_SIZE = 19.773; // 16.4775'in %20 büyütülmüşü (kullanıcı kararı)
  const SPEED = 4.2;         // önceki 1.4'ün 3 katı (kullanıcı kararı)
  const FPS = 25;            // kullanıcı kararı: 15'ten 25'e, maks bu kalsın
  const FRAME_MS = 1000 / FPS;

  const EDGE_SPEED_BONUS = 0.5;
  const SPEED_JITTER_MIN = 0.75;
  const SPEED_JITTER_RANGE = 0.75;
  const FADE_DIST_MID = 0.3125; // 0.25'in %25 artırılmışı — damla daha uzun yaşıyor
  const FADE_DIST_EDGE = 1.25;  // 1.0'ın %25 artırılmışı
  const FLIP_CHANCE = 0.075;

  const HEAD = "0, 255, 120";
  const TAIL = "0, 200, 100";

  function edgeness(x, w) {
    if (w <= 0) return 0;
    return Math.min(1, Math.abs((x / w) * 2 - 1));
  }

  function makeDrop(w, h) {
    const len = 3 + Math.floor(Math.random() * 4);
    const chars = Array.from({ length: len }, () => (Math.random() > 0.5 ? "1" : "0"));

    const columns = Math.max(1, Math.floor(w / CHAR_SIZE));
    const x = Math.floor(Math.random() * columns) * CHAR_SIZE;

    const e = edgeness(x, w);
    const base = SPEED * (1 + EDGE_SPEED_BONUS * e);
    const speed = base * (SPEED_JITTER_MIN + Math.random() * SPEED_JITTER_RANGE);

    return {
      x,
      y: -len * CHAR_SIZE - Math.random() * h * 0.5,
      len,
      chars,
      speed,
      alpha: 0.4 + Math.random() * 0.6,
      fade: Math.max(1, h * (FADE_DIST_MID + (FADE_DIST_EDGE - FADE_DIST_MID) * e)),
    };
  }

  onMount(() => {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced) return;
    visible = true;

    const narrow = window.matchMedia("(max-width: 767px)").matches;
    // Kullanıcı kararı: 10x denendi, sonra %30'a çekildi — net sonuç app'in kendi
    // tuning'inin 3 katı (180→540, 70→210). Karakterler %100 büyüdüğü için görsel
    // yoğunluk hâlâ belirgin şekilde artmış durumda.
    const cols = narrow ? 210 : 540;

    let ctx;
    let cleanup = () => {};

    // Canvas bind sonrası bir mikrotask geriden gelir; effect yerine burada bekliyoruz.
    queueMicrotask(() => {
      if (!canvas) return;
      ctx = canvas.getContext("2d");
      if (!ctx) return;

      const resize = () => {
        canvas.width = canvas.offsetWidth * devicePixelRatio;
        canvas.height = canvas.offsetHeight * devicePixelRatio;
        ctx.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
      };
      resize();
      const obs = new ResizeObserver(resize);
      obs.observe(canvas);

      const w = canvas.offsetWidth;
      const h = canvas.offsetHeight;
      let drops = Array.from({ length: cols }, () => makeDrop(w, h));

      animId = window.setInterval(() => {
        const cw = canvas.offsetWidth;
        const ch = canvas.offsetHeight;
        if (!cw || !ch) return;

        ctx.clearRect(0, 0, cw, ch);
        ctx.font = `600 ${CHAR_SIZE}px "Cascadia Code", "Consolas", ui-monospace, monospace`;

        for (const d of drops) {
          const life = Math.max(0, 1 - Math.max(0, d.y) / d.fade);
          if (life > 0) {
            for (let i = 0; i < d.chars.length; i++) {
              const cy = d.y + i * CHAR_SIZE;
              if (cy < -CHAR_SIZE || cy > ch + CHAR_SIZE) continue;
              const isHead = i === d.chars.length - 1;
              const tailFade = isHead ? d.alpha : d.alpha * Math.max(0.1, 1 - (d.chars.length - 1 - i) * 0.2);
              const a = tailFade * life;
              ctx.fillStyle = isHead
                ? `rgba(${HEAD}, ${Math.min(a, 0.85)})`
                : `rgba(${TAIL}, ${Math.max(a * 0.4, 0)})`;
              ctx.fillText(d.chars[i], d.x, cy);
            }
          }
          d.y += d.speed;
          if (Math.random() < FLIP_CHANCE) {
            d.chars[Math.floor(Math.random() * d.chars.length)] = Math.random() > 0.5 ? "1" : "0";
          }
          if (life <= 0 || d.y - d.len * CHAR_SIZE > ch) {
            Object.assign(d, makeDrop(cw, ch));
          }
        }
      }, FRAME_MS);

      cleanup = () => {
        clearInterval(animId);
        animId = 0;
        obs.disconnect();
      };
    });

    return () => cleanup();
  });
</script>

{#if visible}
  <canvas bind:this={canvas} class="rain" aria-hidden="true"></canvas>
{/if}

<style>
  .rain {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    opacity: 0.22464; /* 0.1872 + %20 */
    mix-blend-mode: screen;
  }
</style>
