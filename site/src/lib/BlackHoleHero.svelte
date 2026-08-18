<script>
  // Uygulamadaki BlackHole.svelte'in DEKORATİF kopyası (orijinale dokunulmadı).
  // Kara delik shader'ı birebir aynı. Eklenenler:
  //   - uZoom: kaydırma ilerlemesiyle delik büyük başlangıçtan (ZOOM_START)
  //     dinlenme boyutuna (ZOOM_REST) küçülür.
  //   - Konum: "evorift" + rozet + başlık artık tek grup (+page.svelte'deki
  //     .hero-copy), viewport merkezi etrafında ortalı. progress=0'da (kaydırılmamış)
  //     delik de aynı merkezde — dy=0, kabaca çakışık (kullanıcı kararı: metin
  //     üzerine gelirse artık şeffaf DOM metni yerine BlackHoleHero'nun kendi 2D
  //     canvas overlay'i tersine çevrilmiş halini çiziyor). Kaydırdıkça (0→P_SHRINK),
  //     zoom'la AYNI eğriyle "yavaşça" GAP_VH kadar yukarı kayar — sürekli metnin
  //     üzerinde durmaz.
  //   - Kaydırma HIZININ ek eğim (tilt) katkısı, mouse pitch'inin üzerine offset
  //     olarak eklenir; hız sönümlenince offset de sıfıra döner.
  //   - Yığılma diski binary karakterlerden (çalışma anında üretilen 0/1 atlasından
  //     örnekleniyor, dosya yok).
  //
  // Binary yağmuru burada DEĞİL — ayrı bir bileşen (BinaryRainHero.svelte), uygulamanın
  // kendi BinaryRain.svelte'inden doğrudan port edilmiş. Kullanıcı kararı (2026-08-17):
  // "uygulamadaki svelte'ten kopyala" — shader'a gömülü ilk deneme yerine.
  //
  // Statik (rAF'sız) mod YALNIZ gerçek prefers-reduced-motion'a bağlı. Önceki sürüm
  // dar pencereyi de statik moda sokuyordu (`matchMedia("max-width:767px")` mount
  // anında kontrol edilip kalıcı hale geliyordu) — tam ekran olmayan, yan panelli ya
  // da dev tools açık bir masaüstü penceresi bu eşiğin altına kolayca düşer, ve o
  // pencere sonradan büyüse bile bileşen o oturum boyunca tek karede kalırdı. Bu,
  // "kara delik oynamıyor" şikayetinin kök nedeniydi.
  //
  // Three.js ASENKRON yüklenir. Yüklenemezse ya da WebGL yoksa bu bileşen sessizce
  // hiçbir şey çizmez; sayfanın içeriği bundan etkilenmez (canvas tamamen dekoratif).
  import { onMount } from "svelte";

  let host;
  let raf = 0;
  let fps = 0;

  // Kaydırma: listener SADECE yazar, rAF SADECE okur ve yumuşatır.
  let scrollRaw = 0;
  let progress = 0;

  // İç render tamponu artık CSS boyutuyla BİREBİR (kullanıcı kararı: "ekranda
  // kapladığı çözünürlük kadar" — önceki 512 tavanı bulanık/bloklu görünüyordu).
  // Bunun bedeli fps: 900+'dan tekrar ~70'e düşer (bkz. skill notu), ama kullanıcı
  // burada görsel netliği fps'in önüne koydu.
  // %75 daha büyük başlasın (kullanıcı kararı): apparent boyut ~1/uZoom ile
  // orantılı, o yüzden başlangıç zoom'u 1/1.75'e düşürüldü (eskiden 1 idi).
  const ZOOM_START = 1 / 1.75;
  // Dinlenme boyutu %20 büyütüldü (kullanıcı kararı): apparent boyut ~1/uZoom ile
  // orantılı, o yüzden 2.35 yerine 2.35/1.2.
  const ZOOM_REST = 2.35 / 1.2; // ≈ 1.9583
  const P_SHRINK = 0.35;      // küçülme bu ilerlemede tamamlanır; içerik reveal'i de buna bağlı
  function smooth01(t) { t = Math.max(0, Math.min(1, t)); return t * t * (3 - 2 * t); }
  // Dinlenme konumunda delik, kendi (büyütülmüş) dikey boyutunun yarısı kadar
  // AŞAĞI kaydırılır (kullanıcı kararı). Ölçüm: piksel-örnekleme ile deliğin
  // dinlenme zoom'undaki gerçek görünür dikey uzanımı bulunup yarısı alındı.
  let settledExtraDownPx = 0;
  // Kullanıcı kararı (son tur): "en son yerleştiği konumdan" (yani yukarıdaki
  // GAP_VH + settledExtraDownPx uygulandıktan SONRAKİ konumdan) bir de kendi
  // dikey boyutunun 1/3'ü kadar YUKARI kaldırılsın — aynı ölçümden türetiliyor
  // (extent/3), tahmin değil.
  let settledExtraUpPx = 0;

  // Kullanıcı kararı (2026-08-18, üçüncü tur): konum artık SCROLL'A bağlı, sabit
  // zamanlı giriş animasyonu değil. Sayfa açılışında (progress=0) delik "evorift"le
  // TAM ÇAKIŞIK durur — dy=0, "ekrana ortalı" — reversal (mix-blend-mode:difference)
  // burada görünür hale gelir. Kaydırdıkça (0→P_SHRINK) dy, aynı zoom eğrisiyle
  // birlikte "yavaşça" GAP_VH kadar yukarı kayar; sürekli metnin üzerinde durmaz.
  const GAP_VH = 0.24;

  // Kaydırma HIZININ ek eğim katkısı — mouse pitch'inin üzerine offset olarak
  // eklenir, ikisi aynı anda etkili olur. Hız sıfırlanınca offset de sıfıra döner
  // (aşağıdaki EMA sönümü sayesinde), mouse'un kendi konumu bundan etkilenmez.
  const SCROLL_TILT_K = 0.00028;  // (piksel/sn hız) -> derece çarpanı
  const SCROLL_TILT_MAX_DEG = 16;

  // sabitlenmiş ayarlar
  const rollDeg = -15;
  const ringR = 8.9;
  const blackR = 5.7;
  const yawDeg = 47;
  const pitchBaseDeg = 15;
  const pitchUpDeg = 13;
  const pitchDnDeg = 4;

  const VERT = `
    void main(){ gl_Position = vec4(position.xy, 0.0, 1.0); }`;

  const FRAG = `
    precision highp float;
    uniform vec2  uRes;
    uniform float uTime;
    uniform float uYaw;
    uniform float uPitch;
    uniform float uActive;
    uniform float uReveal;
    uniform float uRoll;
    uniform float uZoom;

    const int   STEPS  = 300;
    const float DT     = 0.10;
    const float RS     = 1.0;
    const float DIN    = 2.2;
    const float DOUT   = 6.0;
    const float ESCAPE = 30.0;

    mat3 rotX(float a){ float c=cos(a), s=sin(a); return mat3(1.,0.,0., 0.,c,-s, 0.,s,c); }
    mat3 rotY(float a){ float c=cos(a), s=sin(a); return mat3(c,0.,s, 0.,1.,0., -s,0.,c); }

    float diskMask(float r){
      float inner = smoothstep(DIN, DIN + 0.18, r);
      float outer = smoothstep(DOUT, DOUT - 0.35, r);
      return inner * outer;
    }

    void main(){
      vec2 uv = (gl_FragCoord.xy * 2.0 - uRes) / uRes.y;
      uv *= uZoom;
      float cr = cos(uRoll), sr = sin(uRoll);
      uv = mat2(cr, -sr, sr, cr) * uv;

      mat3 R = rotY(uYaw) * rotX(uPitch);
      vec3 ro = R * vec3(0.0, 0.0, 18.0);
      vec3 rd = R * normalize(vec3(uv, -2.0));

      vec3  pos = ro;
      vec3  vel = rd;
      vec3  cprod = cross(pos, vel);
      float h2 = dot(cprod, cprod);

      vec3  col   = vec3(0.0);
      float minR  = 100.0;
      float transmit = 1.0;
      float hitHorizon = 0.0;
      vec3  oldpos = pos;
      float diskGain = clamp(uActive, 0.0, 1.0);

      for (int i = 0; i < STEPS; i++){
        oldpos = pos;
        float r2o = dot(pos, pos);
        float ro_ = sqrt(max(r2o, 1e-6));
        float dt  = DT * clamp(ro_ * 0.35, 0.35, 1.6);

        pos += vel * dt;
        float r2 = dot(pos, pos);
        float r  = sqrt(max(r2, 1e-6));

        vec3  acc = -1.5 * h2 * pos / pow(max(r2, 1e-6), 2.5);
        float am  = min(length(acc), 50.0);
        acc = am * normalize(acc + vec3(1e-9));
        vel += acc * dt;
        vel = normalize(vel);

        if (r < RS){ hitHorizon = 1.0; transmit = 0.0; break; }
        minR = min(minR, r);

        if (oldpos.y * pos.y < 0.0){
          float lambda = oldpos.y / (oldpos.y - pos.y);
          vec3  hit = mix(oldpos, pos, lambda);
          float rr  = length(hit.xz);
          float m   = diskMask(rr);
          if (m > 0.0){
            float a = clamp(m * diskGain, 0.0, 1.0);
            col      += vec3(1.0) * a * transmit;
            transmit *= (1.0 - a);
          }
        }

        if (transmit < 0.004) break;
        if (r > ESCAPE) break;
      }

      float a = max(hitHorizon, 1.0 - transmit);
      a = clamp(a, 0.0, 1.0);
      col = clamp(col, 0.0, 1.0);

      float rdr = abs(minR - 1.5);
      float ringCore = smoothstep(0.16, 0.0, rdr);
      float ringGlow = smoothstep(0.55, 0.0, rdr) * 0.22;
      float ring = (ringCore + ringGlow) * (1.0 - hitHorizon);
      vec3  outRGB = col * a + vec3(ring);
      float outA   = clamp(max(a, ring), 0.0, 1.0);
      gl_FragColor = vec4(outRGB, outA) * uReveal;
    }`;

  const PROJ = `
    uniform float uYaw, uPitch, uAspect, uRoll, uZoom;
    const float CAMD = 18.0;
    mat3 rotX(float a){ float c=cos(a), s=sin(a); return mat3(1.,0.,0., 0.,c,-s, 0.,s,c); }
    mat3 rotY(float a){ float c=cos(a), s=sin(a); return mat3(c,0.,s, 0.,1.,0., -s,0.,c); }
    vec4 project(vec3 P){
      vec3 Pc = rotX(-uPitch) * rotY(-uYaw) * P;
      vec2 uv = 2.0 * Pc.xy / (CAMD - Pc.z);
      float cr = cos(uRoll), sr = sin(uRoll);
      uv = mat2(cr, sr, -sr, cr) * uv;
      uv /= uZoom;
      uv.x /= uAspect;
      return vec4(uv, 0.0, 1.0);
    }`;

  // ---- Yığılma diski: parçacıklar binary karakter ----
  const PART_VERT = PROJ + `
    uniform float uTime, uActive, uRing, uBlackR, uLife;
    attribute float aSeed, aIndex, aBlack, aGlyph;
    varying float vA, vBlack, vGlyph;
    float hash(float n){ return fract(sin(n) * 43758.5453); }
    void main(){
      float ph  = uTime / uLife + aSeed;
      float cyc = floor(ph);
      float life = fract(ph);
      float h1 = hash(cyc + aIndex * 1.7);
      float h2 = hash(cyc * 1.31 + aIndex * 2.9 + 5.0);
      float baseR = (aBlack > 0.5) ? uBlackR : uRing;
      float r0 = baseR * mix(0.68, 1.12, h1 * h1);
      float r  = r0 * (1.0 - life * 0.05);
      float spd = min(7.0 * pow(r0, -1.5), 0.8);
      float ang = h2 * 6.2831853 + uTime * spd;
      vec3 P = vec3(cos(ang) * r, 0.0, sin(ang) * r);
      gl_Position = project(P);
      vec3 Pc = rotX(-uPitch) * rotY(-uYaw) * P;
      float depth = CAMD - Pc.z;
      float sr = length(2.0 * Pc.xy / depth) / uZoom;
      float behind = smoothstep(0.05, -0.05, Pc.z);
      float inSil  = smoothstep(0.56 / uZoom, 0.368 / uZoom, sr);
      float occ = behind * inSil;
      float grow = sin(life * 3.14159265);
      gl_PointSize = (0.4 + 3.6 * grow) * 1.7 * 2.6 / uZoom;
      vA = grow * smoothstep(0.0, 0.2, uActive) * (1.0 - occ);
      vBlack = aBlack;
      vGlyph = aGlyph;
    }`;
  const PART_FRAG = `
    precision mediump float;
    uniform sampler2D uAtlas;
    varying float vA, vBlack, vGlyph;
    void main(){
      // gl_PointCoord.y kaynağı ÜSTTEN (0=üst), CanvasTexture'ın V ekseni ise
      // Three.js'in flipY=true varsayılanıyla ALTTAN (0=alt) — ters düşüyordu.
      // "0" simetrik olduğu için görünmüyordu, "1" baş aşağı çıkıyordu.
      vec2 g = vec2(gl_PointCoord.x * 0.5 + vGlyph * 0.5, 1.0 - gl_PointCoord.y);
      float m = texture2D(uAtlas, g).a;
      float a = m * vA;
      if (a < 0.01) discard;
      vec3 col = mix(vec3(1.0), vec3(0.0), vBlack);
      gl_FragColor = vec4(col * a, a);
    }`;

  /** "0" ve "1" karakterlerinden iki hücreli atlas — dosya yok, çalışma anında üretilir. */
  function glyphAtlas(THREE) {
    const cell = 64;
    const c = document.createElement("canvas");
    c.width = cell * 2;
    c.height = cell;
    const g = c.getContext("2d");
    g.clearRect(0, 0, c.width, c.height);
    g.fillStyle = "#fff";
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.font = `700 ${Math.round(cell * 0.78)}px "Cascadia Code", "Consolas", ui-monospace, monospace`;
    g.fillText("0", cell * 0.5, cell * 0.54);
    g.fillText("1", cell * 1.5, cell * 0.54);
    const tex = new THREE.CanvasTexture(c);
    tex.minFilter = THREE.LinearFilter;
    tex.magFilter = THREE.LinearFilter;
    tex.generateMipmaps = false;
    return tex;
  }

  /** Bir elemanın metnini, tarayıcının GERÇEKTEN sardığı satırlara böler — kendi
      satır-sarma mantığımızı yazıp CSS'in clamp()/dil/genişliğine göre değişen
      sarmayı yeniden icat etmek yerine, Range API ile tarayıcının kendi düzen
      motorunu soruyoruz: her karakter için ayrı bir Range açıp ekran konumunu
      okuyoruz, aynı "top"a sahip olanlar aynı satırdır. Karakter sayısı küçük
      (wordmark/başlık), maliyeti önemsiz; yalnız resize/dil değişiminde çağrılır,
      her karede değil. */
  function getLineSegments(el) {
    if (!el || !el.firstChild || el.firstChild.nodeType !== 3) return [];
    const text = el.firstChild.textContent;
    const range = document.createRange();
    const lines = [];
    let cur = null;
    for (let i = 0; i < text.length; i++) {
      range.setStart(el.firstChild, i);
      range.setEnd(el.firstChild, i + 1);
      const r = range.getClientRects()[0];
      if (!r || r.width === 0) continue;
      if (!cur || Math.abs(r.top - cur.top) > 2) {
        cur = { top: r.top, left: r.left, bottom: r.bottom, text: "" };
        lines.push(cur);
      }
      cur.left = Math.min(cur.left, r.left);
      cur.bottom = Math.max(cur.bottom, r.bottom);
      cur.text += text[i];
    }
    return lines;
  }

  onMount(() => {
    // TEK gerçek statik-mod tetiği: erişilebilirlik tercihi. Genişlik burada ARTIK
    // kullanılmıyor — bkz. dosya başındaki not.
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    let dead = false;
    let cleanup = () => {};

    (async () => {
      let THREE;
      try {
        THREE = await import("$lib/three-lite.js");
      } catch {
        return; // sahne gelmezse sayfa aynen çalışır
      }
      if (dead || !host) return;

      let renderer;
      try {
        renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, premultipliedAlpha: true });
      } catch {
        return; // WebGL yok
      }

      renderer.setPixelRatio(1);
      renderer.setClearColor(0x000000, 0);
      host.appendChild(renderer.domElement);

      // Metin "reversal"ı: CSS mix-blend-mode:difference WebGL canvas'a karşı
      // GÜVENİLMEZ (tarayıcı çoğu zaman canvas'ı ayrı bir compositing katmanına
      // alır, blend o katmanın içeriğini göremez — metin hep düz beyaz kalır,
      // beyaz halkanın üzerinde görünmez olur). Kullanıcı kararı: kendi "shader"ımızı
      // yap. Çözüm: WebGL canvas'ın o anki karesini 2D canvas'a kopyala, üstüne
      // globalCompositeOperation="difference" ile metni çiz — bu, tarayıcının 2D
      // canvas compositing'i, tek bağlamda, güvenilir şekilde piksel piksel
      // tersine çevirir. "evorift" ve başlık için geçerli; ikisi de deliğe değebilen
      // metinler. hero-sub/platform şu an deliğe hiç değmiyor, CSS blend'de kalıyor.
      const textCanvas = document.createElement("canvas");
      textCanvas.style.cssText = "position:absolute;inset:0;width:100%;height:100%;pointer-events:none;display:block;";
      host.appendChild(textCanvas);
      const textCtx = textCanvas.getContext("2d", { willReadFrequently: false });

      let wordmarkLines = [], titleLines = [];
      function measureTextLines() {
        wordmarkLines = getLineSegments(document.querySelector(".wordmark"));
        titleLines = getLineSegments(document.querySelector(".hero-title"));
      }

      /** Deliğin O ANKİ karesini kopyalayıp üstüne "difference" modunda metni
          çizer — metin her zaman altındaki gerçek pikselin tam tersi olur.
          measureTextLines() BURADA, HER ÇAĞRIDA tazeleniyor — .hero-copy artık
          --p'ye göre kayıyor (bkz. +page.svelte), yalnız mount/resize/dil
          değişiminde ölçmek ESKİ (kayma öncesi) konumu dondurup çizerdi: metin
          gerçek DOM'dan ayrı bir yerde "asılı" görünürdü ("arkada kayboluyor"
          şikayetinin sebebi buydu). Range API ölçümü ucuz (kısa string'ler),
          60fps tavanı altında sorun değil. */
      function drawTextOverlay() {
        measureTextLines();
        const w = host.clientWidth, h = host.clientHeight;
        if (w < 2 || h < 2) return;
        if (textCanvas.width !== w || textCanvas.height !== h) {
          textCanvas.width = w;
          textCanvas.height = h;
        }
        textCtx.clearRect(0, 0, w, h);
        textCtx.drawImage(renderer.domElement, 0, 0, w, h);

        textCtx.globalCompositeOperation = "difference";
        textCtx.fillStyle = "#fff";
        textCtx.textAlign = "left";
        textCtx.textBaseline = "bottom";

        const hostRect = host.getBoundingClientRect();
        const drawLines = (el, lines) => {
          if (!el || !lines.length) return;
          const cs = getComputedStyle(el);
          textCtx.font = `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
          for (const line of lines) {
            textCtx.fillText(line.text.trim(), line.left - hostRect.left, line.bottom - hostRect.top);
          }
        };
        drawLines(document.querySelector(".wordmark"), wordmarkLines);
        drawLines(document.querySelector(".hero-title"), titleLines);

        textCtx.globalCompositeOperation = "source-over";
      }

      const scene = new THREE.Scene();
      const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
      const t0 = performance.now();
      const atlas = glyphAtlas(THREE);

      const geo = new THREE.PlaneGeometry(2, 2);
      const mat = new THREE.ShaderMaterial({
        uniforms: {
          uRes: { value: new THREE.Vector2(1, 1) }, uTime: { value: 0 },
          uYaw: { value: 0 }, uPitch: { value: 0.25 },
          uActive: { value: 0 }, uReveal: { value: 0 }, uRoll: { value: 0 },
          uZoom: { value: 1 },
        },
        vertexShader: VERT, fragmentShader: FRAG,
        transparent: true, depthTest: false, depthWrite: false,
        premultipliedAlpha: true, blending: THREE.NormalBlending,
      });
      scene.add(new THREE.Mesh(geo, mat));

      const N = 198;
      const pg = new THREE.BufferGeometry();
      const aSeed = new Float32Array(N), aIndex = new Float32Array(N);
      const aBlack = new Float32Array(N), aGlyph = new Float32Array(N);
      for (let i = 0; i < N; i++) {
        aSeed[i] = Math.random();
        aIndex[i] = i + 1;
        aBlack[i] = i >= N - 30 ? 1 : 0;
        aGlyph[i] = Math.random() < 0.5 ? 0 : 1;
      }
      pg.setAttribute("position", new THREE.BufferAttribute(new Float32Array(N * 3), 3));
      pg.setAttribute("aSeed", new THREE.BufferAttribute(aSeed, 1));
      pg.setAttribute("aIndex", new THREE.BufferAttribute(aIndex, 1));
      pg.setAttribute("aBlack", new THREE.BufferAttribute(aBlack, 1));
      pg.setAttribute("aGlyph", new THREE.BufferAttribute(aGlyph, 1));
      const pMat = new THREE.ShaderMaterial({
        uniforms: {
          uTime: { value: 0 }, uActive: { value: 0 }, uAspect: { value: 1 }, uLife: { value: 3.5 },
          uYaw: { value: 0 }, uPitch: { value: 0.25 }, uRoll: { value: 0 }, uZoom: { value: 1 },
          uRing: { value: ringR }, uBlackR: { value: blackR }, uAtlas: { value: atlas },
        },
        vertexShader: PART_VERT, fragmentShader: PART_FRAG,
        transparent: true, depthTest: false, depthWrite: false,
        premultipliedAlpha: true, blending: THREE.NormalBlending,
      });
      const points = new THREE.Points(pg, pMat);
      points.renderOrder = 2;
      scene.add(points);

      let mx = 0, my = 0, yaw = 0, pitch = 0.25, active = 0;
      let sized = false;
      let gapPx = 0;

      /** Dinlenme boşluğu — window.innerHeight'a göre, resize'da tazelenir.
          Hiçbir DOM elemanı ölçülmüyor: progress=0'da delik zaten hero-copy
          grubunun (+page.svelte) merkeziyle kabaca çakışık. */
      function measureGap() {
        gapPx = GAP_VH * window.innerHeight;
      }

      /** Ölçü alınamadıysa (element henüz 0×0) hiçbir şeyi ayarlamaz — yoksa 1px'lik
          tampon kalıcı hale gelir. Sayfa görünmezken ResizeObserver tetiklenmediği için
          animate() her karede yeniden dener. */
      function size() {
        const w = host.clientWidth, h = host.clientHeight;
        if (w < 2 || h < 2) return false;
        // Tampon artık CSS boyutuyla birebir — bkz. dosya başındaki not.
        renderer.setSize(w, h, false);
        renderer.domElement.style.width = w + "px";
        renderer.domElement.style.height = h + "px";
        mat.uniforms.uRes.value.set(w, h);
        pMat.uniforms.uAspect.value = w / h;
        sized = true;
        return true;
      }

      /** "Delik kendi dikey boyutunun yarısı kadar aşağıda olsun" (kullanıcı
          kararı) — TAHMİN değil, GERÇEK piksel ölçümü: dinlenme zoom'unda bir
          kare çizip merkez sütunda gl.readPixels ile ilk/son opak satırı bulur,
          farkının yarısını alır. Buffer artık CSS boyutuyla birebir olduğu için
          (bkz. size()) ölçüm doğrudan CSS piksel cinsinden çıkıyor. */
      function measureSettledVerticalExtent() {
        if (!sized) return;
        // active (reveal faktörü) mount'ta 0'dan başlayıp yavaşça 1'e çıkar; bu
        // fonksiyon setup'ın hemen ardından çağrılırsa active≈0 olur ve shader'ın
        // son satırı (gl_FragColor *= uReveal) HER ŞEYİ (alpha dahil) sıfırlar —
        // ölçüm hiç opak piksel bulamaz. Ölçüm için active'i geçici zorluyoruz.
        const keepP = progress, keepActive = active;
        progress = P_SHRINK;
        active = 1;
        draw(1);
        const gl = renderer.getContext();
        const bw = renderer.domElement.width, bh = renderer.domElement.height;
        const cx = Math.floor(bw / 2);
        const px = new Uint8Array(bh * 4);
        gl.readPixels(cx, 0, 1, bh, gl.RGBA, gl.UNSIGNED_BYTE, px);
        let top = -1, bottom = -1;
        for (let row = 0; row < bh; row++) {
          if (px[row * 4 + 3] > 10) {
            if (top < 0) top = row;
            bottom = row;
          }
        }
        progress = keepP;
        active = keepActive;
        if (top >= 0 && bottom > top) {
          const extent = bottom - top;
          settledExtraDownPx = extent / 2;
          settledExtraUpPx = extent / 3;
        }
      }

      /** Tek kare çiz — hem canlı döngü hem statik mod bunu kullanır.
          scrollVel: piksel/sn kaydırma hızı — yalnız mouse pitch'inin üzerine
          eklenen bir eğim offseti üretir, kendisi bir konum/zoom girdisi değil. */
      function draw(t, scrollVel = 0) {
        // Aşama 1 (0→P_SHRINK): büyük başlangıçtan dinlenme boyutuna küçül.
        const zoom = progress <= P_SHRINK
          ? ZOOM_START + (ZOOM_REST - ZOOM_START) * smooth01(progress / P_SHRINK)
          : ZOOM_REST;

        // progress=0'da dy=0 (wordmark'la çakışık — reversal burada görünür).
        // Kaydırdıkça (0→P_SHRINK), zoom'la AYNI eğriyle, "yavaşça" -gapPx'e
        // yükselir; sürekli metnin üzerinde durmaz. still'de progress zaten
        // P_SHRINK'te sabit, o yüzden aynı formül orada da doğru sonucu verir.
        // settledExtraDownPx: dinlenme konumunda delik kendi (ölçülmüş) dikey
        // boyutunun yarısı kadar AŞAĞI iner. settledExtraUpPx: kullanıcının SON
        // kararı — o "en son yerleştiği konum"dan (yukarıdaki iki terim
        // uygulandıktan SONRA) bir de kendi boyutunun 1/3'ü kadar YUKARI
        // kaldırılır. Üçü de AYNI eğriyle karışıyor ki tek, tutarlı bir hareket
        // gibi hissettirsin.
        const settleT = smooth01(progress / P_SHRINK);
        const settledDy = -gapPx + settledExtraDownPx - settledExtraUpPx;
        const dy = progress <= P_SHRINK ? settledDy * settleT : settledDy;
        host.style.transform = `translateY(${dy.toFixed(1)}px)`;

        const D2R = Math.PI / 180;
        const rollRad = rollDeg * D2R;

        if (!still) {
          const drift = Math.sin(t * 0.12) * 0.06;
          const yawT = mx * (yawDeg * D2R) + drift;
          const tiltDeg = Math.max(-SCROLL_TILT_MAX_DEG, Math.min(SCROLL_TILT_MAX_DEG, scrollVel * SCROLL_TILT_K));
          const pitchT = (pitchBaseDeg * D2R) + (my >= 0 ? my * pitchUpDeg : my * pitchDnDeg) * D2R + tiltDeg * D2R;
          yaw += (yawT - yaw) * 0.06;
          pitch += (pitchT - pitch) * 0.06;
          pitch = Math.max(0, Math.min(1.35, pitch));
          active += (1 - active) * 0.05;
        } else {
          yaw = 0;
          pitch = pitchBaseDeg * D2R;
          active = 1;
        }

        const reveal = Math.max(0, Math.min(1, (active - 0.3) / 0.7));

        mat.uniforms.uTime.value = t;
        mat.uniforms.uYaw.value = yaw;
        mat.uniforms.uPitch.value = pitch;
        mat.uniforms.uActive.value = reveal;
        mat.uniforms.uReveal.value = reveal;
        mat.uniforms.uRoll.value = rollRad;
        mat.uniforms.uZoom.value = zoom;

        pMat.uniforms.uTime.value = t;
        pMat.uniforms.uActive.value = active;
        pMat.uniforms.uYaw.value = yaw;
        pMat.uniforms.uPitch.value = pitch;
        pMat.uniforms.uRoll.value = rollRad;
        pMat.uniforms.uZoom.value = zoom;
        points.visible = active > 0.001;

        renderer.render(scene, camera);
        drawTextOverlay();
      }

      if (import.meta.env.DEV) {
        // Ölçüm kancası (YALNIZ dev build). readPixels GPU'yu bitirmeye zorlar.
        window.__evoriftBench = (p = 0, frames = 60) => {
          if (!sized && !size()) return null;
          const gl = renderer.getContext();
          const px = new Uint8Array(4);
          const keep = progress;
          progress = p;
          draw(1);
          gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
          const start = performance.now();
          for (let i = 0; i < frames; i++) {
            draw(1 + i * 0.016);
            gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
          }
          const ms = (performance.now() - start) / frames;
          progress = keep;
          return {
            progress: p,
            msPerFrame: +ms.toFixed(2),
            fps: Math.round(1000 / ms),
            buffer: [renderer.domElement.width, renderer.domElement.height],
          };
        };
        // Görsel kancası (YALNIZ dev build). Sekme gizliyken bile toDataURL çalışır
        // (compositor'dan değil GL çizim tamponundan okur) — bu, tarayıcı paneli
        // görünmediğinde tek gerçek doğrulama yolu.
        window.__evoriftSnapshot = (p = 0) => {
          const prevSize = [renderer.domElement.width, renderer.domElement.height];
          renderer.setSize(360, 202, false);
          mat.uniforms.uRes.value.set(360, 202);
          pMat.uniforms.uAspect.value = 360 / 202;
          const keep = progress;
          progress = p;
          draw(1.2);
          const url = renderer.domElement.toDataURL("image/png");
          progress = keep;
          renderer.setSize(prevSize[0], prevSize[1], false);
          if (sized) size();
          return url;
        };
      }

      // --- statik mod: tek kare, rAF yok ---
      if (still) {
        // Dinlenme boyutunda — progress=P_SHRINK, aynı dy formülü -gapPx'e sabitler.
        // measureTextLines() ayrıca çağrılmıyor — draw()→drawTextOverlay() zaten
        // her seferinde kendi tazeler.
        progress = P_SHRINK;
        measureGap();
        document.documentElement.classList.add("text-overlay-ready");
        const textObs = new MutationObserver(() => draw(0.8));
        [".wordmark", ".hero-title"].forEach((sel) => {
          const el = document.querySelector(sel);
          if (el) textObs.observe(el, { characterData: true, childList: true, subtree: true });
        });
        const obs = new ResizeObserver(() => { measureGap(); if (size()) { measureSettledVerticalExtent(); draw(0.8); } });
        obs.observe(host);
        if (size()) { measureSettledVerticalExtent(); draw(0.8); }
        cleanup = () => {
          obs.disconnect();
          textObs.disconnect();
          document.documentElement.classList.remove("text-overlay-ready");
          geo.dispose(); mat.dispose(); pg.dispose(); pMat.dispose(); atlas.dispose();
          renderer.dispose(); renderer.forceContextLoss(); renderer.domElement.remove();
          textCanvas.remove();
        };
        return;
      }

      // --- canlı döngü ---
      let frames = 0, fpsT = 0;
      // Kaydırma yumuşatmasının payda için: .hero'nun gerçek yüksekliği kullanılır,
      // window.innerHeight varsayımı değil. 220vh/100vh sticky ise 120vh mesafe kaydırılır;
      // önceki sürüm bunu 100vh sanıyordu, delik ilerlemenin ilk %83'ünde küçülüp kalıyordu.
      let heroTop = 0, span = 1;
      function measureHero() {
        const heroEl = host.closest(".hero") || host;
        const r = heroEl.getBoundingClientRect();
        heroTop = r.top + window.scrollY;
        span = Math.max(1, heroEl.offsetHeight - window.innerHeight);
      }

      let lastScrollForVel = scrollRaw, lastVelT = 0, scrollVel = 0;
      // Delik yerine oturunca (P_SHRINK'i geçince) sayfa geri kalan içeriği/CTA'yı
      // açar — bkz. +page.svelte'deki :root.bh-settled kuralları. Class, değer
      // gerçekten değişince tek sefer yazılır (her karede yazmak gereksiz).
      let settled = false;

      // Kullanıcı kararı: 60fps'te CAP'lensin — yüksek yenileme hızlı ekranlarda
      // (144Hz+) rAF sınırsız çalışırdı, ihtiyaçtan fazla GPU/pil harcardı. rAF
      // yine her tick'te planlanır (zamanlama akışı bozulmasın), ama süre dolmadan
      // gelen kareler HİÇBİR iş yapmadan atlanır — hesaplama da, çizim de yok.
      const MAX_FPS = 60;
      const MIN_FRAME_MS = 1000 / MAX_FPS;
      let lastRenderMs = 0;

      function animate() {
        raf = requestAnimationFrame(animate);
        const nowMs = performance.now();
        if (nowMs - lastRenderMs < MIN_FRAME_MS) return;
        lastRenderMs = nowMs;
        if (!sized && !size()) return;
        const t = (nowMs - t0) / 1000;

        const target = Math.max(0, Math.min(1, (scrollRaw - heroTop) / span));
        progress += (target - progress) * 0.12;
        if (Math.abs(target - progress) < 0.002) progress = target;
        document.documentElement.style.setProperty("--p", progress.toFixed(3));

        const isSettled = progress > P_SHRINK;
        if (isSettled !== settled) {
          settled = isSettled;
          document.documentElement.classList.toggle("bh-settled", settled);
        }

        // Kaydırma HIZI (px/sn), EMA ile yumuşatılmış — durunca kendiliğinden 0'a
        // söner, bu yüzden ayrı bir "geri dön" mantığı gerekmiyor.
        const dt = Math.max(0.001, t - lastVelT);
        const instVel = (scrollRaw - lastScrollForVel) / dt;
        lastScrollForVel = scrollRaw;
        lastVelT = t;
        scrollVel += (instVel - scrollVel) * 0.15;

        draw(t, scrollVel);

        frames++;
        if (t - fpsT >= 1) {
          fps = Math.round(frames / (t - fpsT));
          frames = 0;
          fpsT = t;
          window.__evoriftFps = fps;
        }
      }

      size();
      measureHero();
      measureGap();
      measureSettledVerticalExtent();
      document.documentElement.classList.add("text-overlay-ready");
      // Dil değişimi veya kaydırma sonucu konum değişikliği burada AYRICA izlenmiyor
      // — animate() zaten her karede çalışıp drawTextOverlay() üzerinden
      // measureTextLines()'ı tazeliyor, bir sonraki karede otomatik yansır.
      const obs = new ResizeObserver(() => { size(); measureHero(); measureGap(); measureSettledVerticalExtent(); });
      obs.observe(host);

      const onScroll = () => { scrollRaw = window.scrollY || 0; };
      const onMove = (e) => {
        const r = host.getBoundingClientRect();
        mx = ((e.clientX - r.left) / r.width - 0.5) * 2;
        my = -((e.clientY - r.top) / r.height - 0.5) * 2;
      };
      const onLeave = () => { mx = 0; my = 0; };
      const onVis = () => {
        if (document.hidden) { cancelAnimationFrame(raf); raf = 0; }
        else if (!raf) animate();
      };
      const onResize = () => { measureHero(); measureGap(); };

      onScroll();
      window.addEventListener("scroll", onScroll, { passive: true });
      window.addEventListener("resize", onResize, { passive: true });
      host.addEventListener("pointermove", onMove);
      host.addEventListener("pointerleave", onLeave);
      document.addEventListener("visibilitychange", onVis);
      animate();

      cleanup = () => {
        cancelAnimationFrame(raf);
        obs.disconnect();
        document.documentElement.classList.remove("text-overlay-ready");
        window.removeEventListener("scroll", onScroll);
        window.removeEventListener("resize", onResize);
        host.removeEventListener("pointermove", onMove);
        host.removeEventListener("pointerleave", onLeave);
        document.removeEventListener("visibilitychange", onVis);
        geo.dispose(); mat.dispose(); pg.dispose(); pMat.dispose(); atlas.dispose();
        renderer.dispose(); renderer.forceContextLoss(); renderer.domElement.remove();
        textCanvas.remove();
      };
    })();

    return () => { dead = true; cleanup(); };
  });
</script>

<!-- Tamamen dekoratif: odak alamaz, ekran okuyucuya görünmez, içerik buna bağlı değil. -->
<div class="bh" bind:this={host} aria-hidden="true"></div>

<style>
  .bh { position: relative; width: 100%; height: 100%; }
  :global(.bh canvas) {
    position: absolute;
    inset: 0;
    display: block;
    width: 100% !important;
    height: 100% !important;
  }
</style>
