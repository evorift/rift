<script>
  // Uygulamadaki BlackHole.svelte'in DEKORATİF kopyası (orijinale dokunulmadı).
  // Kara delik shader'ı birebir aynı; eklenenler:
  //   1. uZoom      — kaydırma ilerlemesiyle delik küçülür (dinlenme konumuna oturur).
  //   2. Binary yağmuru — AYNI sahnede, ayrı canvas değil. Ufka yakın bükülür, deliğe
  //      girince söner. Yoğunluk yatay konuma bağlı: kenarlar yoğun, merkez seyrek.
  //   3. Yığılma diski binary karakterlerden — yutulan engeller diskin malzemesi olur.
  //
  // Three.js ASENKRON yüklenir. Yüklenemezse ya da WebGL yoksa bu bileşen sessizce
  // hiçbir şey çizmez; sayfanın içeriği bundan etkilenmez (canvas tamamen dekoratif).
  import { onMount } from "svelte";

  let {
    // Kaydırmanın kaç piksellik kısmı koreografiye ayrılmış olsun.
    range = 0,
  } = $props();

  let host;
  let raf = 0;
  let fps = 0;

  // Kaydırma: listener SADECE yazar, rAF SADECE okur ve yumuşatır.
  let scrollRaw = 0;
  let progress = 0;

  const SCALE = 0.9;          // iç tampon ölçeği (mevcut koddan korundu)
  const ZOOM_REST = 2.35;     // dinlenme konumunda delik bu kadar küçülür
  const RAIN_N = 900;

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

  // ---- Yığılma diski: parçacıklar artık binary karakter ----
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
      vec2 g = vec2(gl_PointCoord.x * 0.5 + vGlyph * 0.5, gl_PointCoord.y);
      float m = texture2D(uAtlas, g).a;
      float a = m * vA;
      if (a < 0.01) discard;
      vec3 col = mix(vec3(1.0), vec3(0.0), vBlack);
      gl_FragColor = vec4(col * a, a);
    }`;

  // ---- Binary yağmuru: aynı sahnede, ufka yakın bükülür, deliğe girince söner ----
  // Konum uv uzayında hesaplanır (kara delik shader'ıyla aynı uzay), sonra NDC'ye çevrilir.
  const RAIN_VERT = `
    uniform float uTime, uAspect, uZoom, uLens, uFlow, uGlyphPx, uRainA;
    attribute float aCol, aSeed, aSpeed, aGlyph;
    varying float vA, vGlyph;
    void main(){
      // Düşüş. uFlow=1 iken hızlı ve düz akar (engel yok), 0 iken yavaş ve savruk.
      float speed = aSpeed * mix(0.55, 1.35, uFlow);
      float y = 1.25 - fract(uTime * speed + aSeed) * 2.5;

      // Savrulma yalnız akış serbest değilken var — düzeldikçe sıfırlanır.
      float sway = (1.0 - uFlow) * 0.055 * sin(uTime * 0.7 + aSeed * 31.0);
      vec2 p = vec2(aCol * uAspect + sway, y);

      // Ufka yakın bükülme (kütleçekimsel mercek). Delik küçüldükçe etkisi kaybolur.
      float horizon = 0.30 / uZoom;
      float d = max(length(p), 1e-4);
      float pull = uLens * 0.055 / max(d * d, 0.015);
      p -= (p / d) * min(pull, d * 0.85);

      // Deliğe giren sönüyor: yutuldu.
      float d2 = length(p);
      float eaten = smoothstep(horizon * 0.85, horizon * 1.9, d2);

      // Yoğunluk yatay konuma bağlı: kenarlar tam, merkez seyrek — yumuşak eğri.
      float edge = pow(abs(aCol), 0.75);
      float dens = mix(0.10, 1.0, edge);

      // Üst ve alt kenarda yumuşak giriş/çıkış.
      float fade = smoothstep(1.25, 1.0, abs(y)) ;

      gl_Position = vec4(p.x / uAspect, p.y, 0.0, 1.0);
      gl_PointSize = uGlyphPx;
      vA = dens * eaten * fade * uRainA;
      vGlyph = aGlyph;
    }`;
  const RAIN_FRAG = `
    precision mediump float;
    uniform sampler2D uAtlas;
    uniform vec3 uColor;
    varying float vA, vGlyph;
    void main(){
      vec2 g = vec2(gl_PointCoord.x * 0.5 + vGlyph * 0.5, gl_PointCoord.y);
      float m = texture2D(uAtlas, g).a;
      float a = m * vA;
      if (a < 0.01) discard;
      gl_FragColor = vec4(uColor * a, a);
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

  onMount(() => {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const mobile = window.matchMedia("(max-width: 767px)").matches;
    // reduced-motion ve mobil: hiç animasyon yok, delik dinlenme konumunda tek kare.
    const still = reduced || mobile;

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

      const scene = new THREE.Scene();
      const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
      const t0 = performance.now();
      const atlas = glyphAtlas(THREE);

      // --- kara delik ---
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
      const holeMesh = new THREE.Mesh(geo, mat);
      holeMesh.renderOrder = 0;
      scene.add(holeMesh);

      // --- yağmur: delikten ÖNCE çizilir, böylece silüetin arkasına giren kaybolur ---
      const rg = new THREE.BufferGeometry();
      const rCol = new Float32Array(RAIN_N), rSeed = new Float32Array(RAIN_N);
      const rSpeed = new Float32Array(RAIN_N), rGlyph = new Float32Array(RAIN_N);
      for (let i = 0; i < RAIN_N; i++) {
        // pow(u, 0.45) dağılımı kenarlara doğru yığar — merkez sütun seyrek kalır.
        const u = Math.random();
        rCol[i] = (Math.random() < 0.5 ? -1 : 1) * Math.pow(u, 0.45);
        rSeed[i] = Math.random();
        rSpeed[i] = 0.09 + Math.random() * 0.16;
        rGlyph[i] = Math.random() < 0.5 ? 0 : 1;
      }
      rg.setAttribute("position", new THREE.BufferAttribute(new Float32Array(RAIN_N * 3), 3));
      rg.setAttribute("aCol", new THREE.BufferAttribute(rCol, 1));
      rg.setAttribute("aSeed", new THREE.BufferAttribute(rSeed, 1));
      rg.setAttribute("aSpeed", new THREE.BufferAttribute(rSpeed, 1));
      rg.setAttribute("aGlyph", new THREE.BufferAttribute(rGlyph, 1));
      const rMat = new THREE.ShaderMaterial({
        uniforms: {
          uTime: { value: 0 }, uAspect: { value: 1 }, uZoom: { value: 1 },
          uLens: { value: 1 }, uFlow: { value: 0 }, uGlyphPx: { value: 12 },
          uRainA: { value: 1 }, uAtlas: { value: atlas },
          uColor: { value: new THREE.Color(0x39e66b) },
        },
        vertexShader: RAIN_VERT, fragmentShader: RAIN_FRAG,
        transparent: true, depthTest: false, depthWrite: false,
        premultipliedAlpha: true, blending: THREE.NormalBlending,
      });
      const rain = new THREE.Points(rg, rMat);
      rain.renderOrder = -1;
      scene.add(rain);

      // --- disk parçacıkları (binary) ---
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

      /** Ölçü alınamadıysa (element henüz 0×0) hiçbir şeyi ayarlamaz — yoksa 1px'lik
          tampon kalıcı hale gelir. Sayfa görünmezken ResizeObserver tetiklenmediği için
          animate() her karede yeniden dener. */
      function size() {
        const w = host.clientWidth, h = host.clientHeight;
        if (w < 2 || h < 2) return false;
        const bw = Math.max(1, Math.round(w * SCALE)), bh = Math.max(1, Math.round(h * SCALE));
        renderer.setSize(bw, bh, false);
        renderer.domElement.style.width = w + "px";
        renderer.domElement.style.height = h + "px";
        mat.uniforms.uRes.value.set(bw, bh);
        const aspect = w / h;
        pMat.uniforms.uAspect.value = aspect;
        rMat.uniforms.uAspect.value = aspect;
        rMat.uniforms.uGlyphPx.value = Math.max(8, Math.min(20, bh * 0.019));
        sized = true;
        return true;
      }

      /** Tek kare çiz — hem canlı döngü hem statik mod bunu kullanır. */
      function draw(t) {
        const zoom = 1 + (ZOOM_REST - 1) * progress;
        const D2R = Math.PI / 180;
        const rollRad = rollDeg * D2R;

        if (!still) {
          const drift = Math.sin(t * 0.12) * 0.06;
          const yawT = mx * (yawDeg * D2R) + drift;
          const pitchT = (pitchBaseDeg * D2R) + (my >= 0 ? my * pitchUpDeg : my * pitchDnDeg) * D2R;
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

        // Delik küçüldükçe mercek etkisi biter ve akış düzelir.
        rMat.uniforms.uTime.value = t;
        rMat.uniforms.uZoom.value = zoom;
        rMat.uniforms.uLens.value = 1 - progress;
        rMat.uniforms.uFlow.value = progress;
        rMat.uniforms.uRainA.value = still ? 0.5 : 1;

        renderer.render(scene, camera);
      }

      // --- statik mod: tek kare, rAF yok ---
      if (still) {
        progress = 1;
        const obs = new ResizeObserver(() => { if (size()) draw(0.8); });
        obs.observe(host);
        if (size()) draw(0.8);
        cleanup = () => {
          obs.disconnect();
          geo.dispose(); mat.dispose(); rg.dispose(); rMat.dispose();
          pg.dispose(); pMat.dispose(); atlas.dispose();
          renderer.dispose(); renderer.forceContextLoss(); renderer.domElement.remove();
        };
        return;
      }

      // --- canlı döngü ---
      let frames = 0, fpsT = 0;

      function animate() {
        raf = requestAnimationFrame(animate);
        if (!sized && !size()) return;
        const t = (performance.now() - t0) / 1000;

        // Kaydırma: bir kez okunmuş değeri burada yumuşat.
        const span = range || window.innerHeight || 1;
        const target = Math.max(0, Math.min(1, scrollRaw / span));
        progress += (target - progress) * 0.12;
        if (Math.abs(target - progress) < 0.002) progress = target;
        // CSS tarafı da aynı ilerlemeyi kullanır — ikinci bir rAF/scroll döngüsü yok.
        document.documentElement.style.setProperty("--p", progress.toFixed(3));

        draw(t);

        frames++;
        if (t - fpsT >= 1) {
          fps = Math.round(frames / (t - fpsT));
          frames = 0;
          fpsT = t;
          window.__evoriftFps = fps;
        }
      }

      if (import.meta.env.DEV) {
        // Ölçüm kancası (YALNIZ dev build). Sekme görünmezken rAF durduğu için fps'i
        // senkron çizim + readPixels ile ölçer; readPixels GPU'yu bitirmeye zorlar,
        // yoksa sadece komut kuyruğa atma süresini ölçmüş olurduk.
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
      }

      size();
      const obs = new ResizeObserver(size);
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

      onScroll();
      window.addEventListener("scroll", onScroll, { passive: true });
      host.addEventListener("pointermove", onMove);
      host.addEventListener("pointerleave", onLeave);
      document.addEventListener("visibilitychange", onVis);
      animate();

      cleanup = () => {
        cancelAnimationFrame(raf);
        obs.disconnect();
        window.removeEventListener("scroll", onScroll);
        host.removeEventListener("pointermove", onMove);
        host.removeEventListener("pointerleave", onLeave);
        document.removeEventListener("visibilitychange", onVis);
        geo.dispose(); mat.dispose(); rg.dispose(); rMat.dispose();
        pg.dispose(); pMat.dispose(); atlas.dispose();
        renderer.dispose(); renderer.forceContextLoss(); renderer.domElement.remove();
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
