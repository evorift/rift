<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { app } from "$lib/state.svelte";

  let host: HTMLButtonElement;
  let raf = 0;
  let renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.OrthographicCamera;
  let mat: THREE.ShaderMaterial;
  let pMat: THREE.ShaderMaterial, points: THREE.Points;
  const clock = new THREE.Clock();

  let mx = 0, my = 0;
  let yaw = 0, pitch = 0.25;
  // pcount: parçacıkların teker teker beliriş ilerlemesi (0->1). Disk/ring bundan sonra gelir.
  let pcount = 0, pcountTarget = 0;

  // CANLI AYAR sliderları (değeri okuyup bana söyle)
  let rollDeg = $state(-15);  // ekran-dik eksen eğimi (derece)
  let ringR = $state(7.5);    // beyaz parçacık ring yarıçapı
  let blackR = $state(3.5);   // siyah parçacık yarıçapı (beyaz diskin üstünde)

  const SCALE = 0.9;

  const VERT = `
    void main(){ gl_Position = vec4(position.xy, 0.0, 1.0); }`;

  // ---- Schwarzschild-approx geodesic raymarcher (units: r_s = 1) ----
  const FRAG = `
    precision highp float;
    uniform vec2  uRes;
    uniform float uTime;
    uniform float uYaw;
    uniform float uPitch;
    uniform float uActive;
    uniform float uReveal;
    uniform float uRoll;

    const int   STEPS  = 300;
    const float DT     = 0.10;
    const float RS     = 1.0;
    const float DIN    = 2.2;
    const float DOUT   = 6.0;
    const float ESCAPE = 30.0;
    const float PI     = 3.14159265;

    mat3 rotX(float a){ float c=cos(a), s=sin(a); return mat3(1.,0.,0., 0.,c,-s, 0.,s,c); }
    mat3 rotY(float a){ float c=cos(a), s=sin(a); return mat3(c,0.,s, 0.,1.,0., -s,0.,c); }

    float diskMask(float r){
      float inner = smoothstep(DIN, DIN + 0.18, r);
      float outer = smoothstep(DOUT, DOUT - 0.35, r);
      return inner * outer;
    }

    void main(){
      vec2 uv = (gl_FragCoord.xy * 2.0 - uRes) / uRes.y;
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

      float ring = smoothstep(0.16, 0.0, abs(minR - 1.5)) * (1.0 - hitHorizon);
      vec3  outRGB = col * a + vec3(ring);
      float outA   = clamp(max(a, ring), 0.0, 1.0);
      gl_FragColor = vec4(outRGB, outA) * uReveal;
    }`;

  // kara delik ile AYNI kamera (D=18, focal=2, roll) -> parçacıklar 3B uzayda hizalı
  const PROJ = `
    uniform float uYaw, uPitch, uAspect, uRoll;
    const float CAMD = 18.0;
    mat3 rotX(float a){ float c=cos(a), s=sin(a); return mat3(1.,0.,0., 0.,c,-s, 0.,s,c); }
    mat3 rotY(float a){ float c=cos(a), s=sin(a); return mat3(c,0.,s, 0.,1.,0., -s,0.,c); }
    vec4 project(vec3 P){
      vec3 Pc = rotX(-uPitch) * rotY(-uYaw) * P;
      vec2 uv = 2.0 * Pc.xy / (CAMD - Pc.z);
      float cr = cos(uRoll), sr = sin(uRoll);
      uv = mat2(cr, -sr, sr, cr) * uv;
      uv.x /= uAspect;
      return vec4(uv, 0.0, 1.0);
    }`;

  // tek parçacık sistemi: 30 parçacık, ring üstünde 3B döner, teker teker belirir/yok olur
  const PART_VERT = PROJ + `
    uniform float uTime, uPcount, uRing, uBlackR;
    attribute float aBase, aSeed, aRad, aBlack;
    varying float vA, vBlack;
    void main(){
      float r = (aBlack > 0.5) ? uBlackR * mix(0.8, 1.2, aRad)   // siyah: diskin beyazı üstünde
                               : uRing  * mix(0.82, 1.0, aRad);  // beyaz: ring üzerinde
      float ang = aBase - uTime * 0.4;                            // ters yön (ring ile aynı)
      vec3 P = vec3(cos(ang) * r, 0.0, sin(ang) * r);
      gl_Position = project(P);
      float app = smoothstep(aSeed, aSeed + 0.10, uPcount);       // teker teker beliriş/yok
      gl_PointSize = (0.5 + 3.2 * app) * 1.7;                      // büyüyerek belir / küçülerek yok
      vA = app;
      vBlack = aBlack;
    }`;
  const PART_FRAG = `
    precision mediump float;
    varying float vA, vBlack;
    void main(){
      float d = distance(gl_PointCoord, vec2(0.5));
      float a = smoothstep(0.5, 0.0, d) * vA;
      if (a < 0.01) discard;
      vec3 col = mix(vec3(1.0), vec3(0.0), vBlack);               // beyaz / siyah
      gl_FragColor = vec4(col, a);
    }`;

  $effect(() => {
    pcountTarget = (app.status === "on" || app.status === "connecting") ? 1 : 0;
  });

  function size() {
    const w = host.clientWidth || 1, h = host.clientHeight || 1;
    renderer.setSize(w * SCALE, h * SCALE, false);
    renderer.domElement.style.width = w + "px";
    renderer.domElement.style.height = h + "px";
    mat.uniforms.uRes.value.set(w * SCALE, h * SCALE);
    if (pMat) pMat.uniforms.uAspect.value = w / h;
  }

  function animate() {
    raf = requestAnimationFrame(animate);
    const t = clock.getElapsedTime();
    const drift = Math.sin(t * 0.12) * 0.3;

    // sağ-sol normal (yaw), yukarı-aşağı dikey döndürme (pitch, geniş aralık)
    yaw   += ((mx * 0.9 + drift) - yaw) * 0.05;
    pitch += ((0.25 + my * 0.5) - pitch) * 0.05;
    pitch = Math.max(0.0, Math.min(0.95, pitch));

    // pcount yavaş ilerler -> parçacıklar teker teker
    pcount += (pcountTarget - pcount) * 0.04;
    const reveal = Math.max(0, Math.min(1, (pcount - 0.6) / 0.4)); // disk/ring parçacıklardan SONRA

    const rollRad = (rollDeg * Math.PI) / 180;
    mat.uniforms.uTime.value = t;
    mat.uniforms.uYaw.value = yaw;
    mat.uniforms.uPitch.value = pitch;
    mat.uniforms.uActive.value = reveal;
    mat.uniforms.uReveal.value = reveal;
    mat.uniforms.uRoll.value = rollRad;

    pMat.uniforms.uTime.value = t;
    pMat.uniforms.uPcount.value = pcount;
    pMat.uniforms.uYaw.value = yaw;
    pMat.uniforms.uPitch.value = pitch;
    pMat.uniforms.uRoll.value = rollRad;
    pMat.uniforms.uRing.value = ringR;
    pMat.uniforms.uBlackR.value = blackR;
    points.visible = pcount > 0.001;

    renderer.render(scene, camera);
  }

  onMount(() => {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, premultipliedAlpha: true });
    renderer.setPixelRatio(1);
    renderer.setClearColor(0x000000, 0);
    host.appendChild(renderer.domElement);

    scene = new THREE.Scene();
    camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

    const geo = new THREE.PlaneGeometry(2, 2);
    mat = new THREE.ShaderMaterial({
      uniforms: {
        uRes: { value: new THREE.Vector2(1, 1) }, uTime: { value: 0 },
        uYaw: { value: 0 }, uPitch: { value: 0.25 },
        uActive: { value: 0 }, uReveal: { value: 0 }, uRoll: { value: 0 },
      },
      vertexShader: VERT, fragmentShader: FRAG,
      transparent: true, depthTest: false, depthWrite: false,
      premultipliedAlpha: true, blending: THREE.NormalBlending,
    });
    scene.add(new THREE.Mesh(geo, mat));

    // 30 parçacık: ~22 beyaz (ring) + ~8 siyah (beyaz diskin üstünde)
    const N = 30;
    const pg = new THREE.BufferGeometry();
    const aBase = new Float32Array(N), aSeed = new Float32Array(N), aRad = new Float32Array(N), aBlack = new Float32Array(N);
    const dum = new Float32Array(N * 3);
    for (let i = 0; i < N; i++) {
      aBase[i] = Math.random() * Math.PI * 2;
      aSeed[i] = i / N;                    // teker teker (sıralı eşik)
      aRad[i] = Math.random();
      aBlack[i] = i >= N - 8 ? 1 : 0;      // son 8 siyah
    }
    pg.setAttribute("position", new THREE.BufferAttribute(dum, 3));
    pg.setAttribute("aBase", new THREE.BufferAttribute(aBase, 1));
    pg.setAttribute("aSeed", new THREE.BufferAttribute(aSeed, 1));
    pg.setAttribute("aRad", new THREE.BufferAttribute(aRad, 1));
    pg.setAttribute("aBlack", new THREE.BufferAttribute(aBlack, 1));
    pMat = new THREE.ShaderMaterial({
      uniforms: {
        uTime: { value: 0 }, uPcount: { value: 0 }, uAspect: { value: 1 },
        uYaw: { value: 0 }, uPitch: { value: 0.25 }, uRoll: { value: 0 },
        uRing: { value: 7.5 }, uBlackR: { value: 3.5 },
      },
      vertexShader: PART_VERT, fragmentShader: PART_FRAG,
      transparent: true, depthTest: false, depthWrite: false, blending: THREE.NormalBlending,
    });
    points = new THREE.Points(pg, pMat); points.visible = false; points.renderOrder = 2; scene.add(points);

    size();
    const obs = new ResizeObserver(size); obs.observe(host);
    const onMove = (e: PointerEvent) => {
      const r = host.getBoundingClientRect();
      mx = ((e.clientX - r.left) / r.width - 0.5) * 2;
      my = -((e.clientY - r.top) / r.height - 0.5) * 2;
    };
    host.addEventListener("pointermove", onMove);
    const onVis = () => { if (document.hidden) { cancelAnimationFrame(raf); raf = 0; } else if (!raf) animate(); };
    document.addEventListener("visibilitychange", onVis);
    animate();

    return () => {
      cancelAnimationFrame(raf); obs.disconnect();
      host.removeEventListener("pointermove", onMove);
      document.removeEventListener("visibilitychange", onVis);
      geo.dispose(); mat.dispose(); pg.dispose(); pMat.dispose();
      renderer.dispose(); renderer.forceContextLoss(); renderer.domElement.remove();
    };
  });
</script>

<div class="bh-wrap">
  <button class="bh" bind:this={host} onclick={() => app.toggle()} aria-label="Kara delik · aç/kapat">
    <svg class="power" class:show={app.status === "off"} viewBox="0 0 100 100" aria-hidden="true">
      <circle class="rim" cx="50" cy="50" r="46" />
      <line class="stem" x1="50" y1="27" x2="50" y2="47" />
      <path class="arc" d="M34 37 A20 20 0 1 0 66 37" />
    </svg>
  </button>
  <div class="dbg">
    <label>Roll <b>{rollDeg}°</b><input type="range" min="-45" max="45" step="1" bind:value={rollDeg} /></label>
    <label>Ring <b>{ringR.toFixed(1)}</b><input type="range" min="1" max="14" step="0.1" bind:value={ringR} /></label>
    <label>Black <b>{blackR.toFixed(1)}</b><input type="range" min="1" max="10" step="0.1" bind:value={blackR} /></label>
  </div>
</div>

<style>
  .bh-wrap { position: relative; width: 100%; height: 100%; }
  .dbg {
    position: absolute; top: 6px; left: 6px; z-index: 5; pointer-events: auto;
    display: flex; flex-direction: column; gap: 4px;
    background: rgba(0,0,0,.5); padding: 6px 8px; border-radius: 8px;
  }
  .dbg label { display: flex; align-items: center; gap: 6px; font-size: 10px; color: #cfd6e4; white-space: nowrap; }
  .dbg b { color: #fff; min-width: 36px; display: inline-block; }
  .dbg input { width: 92px; }
  .bh { position: relative; width: 100%; height: 100%; border: none; background: transparent; cursor: pointer; padding: 0; display: block; }
  :global(.bh canvas) { position: absolute; inset: 0; z-index: 1; display: block; width: 100% !important; height: 100% !important; }
  .power {
    position: absolute; inset: 0; margin: auto; width: 60%; height: 60%; z-index: 2;
    pointer-events: none; opacity: 0; transform: scale(.9);
    transition: opacity .3s ease, transform .3s ease;
  }
  .power.show { opacity: 1; transform: scale(1); }
  .power .rim { fill: none; stroke: #fff; stroke-width: 1.6; opacity: .55; }
  .power .stem, .power .arc { fill: none; stroke: #fff; stroke-width: 5; stroke-linecap: round; }
</style>
