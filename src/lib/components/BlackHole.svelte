<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { app } from "$lib/state.svelte";

  let host: HTMLButtonElement;
  let raf = 0;
  let renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.OrthographicCamera;
  let mat: THREE.ShaderMaterial;
  let pMat: THREE.ShaderMaterial, points: THREE.Points;
  let aMat: THREE.ShaderMaterial, aPoints: THREE.Points;
  const clock = new THREE.Clock();

  // mouse-driven orbit targets + lerped state
  let mx = 0, my = 0;
  let yaw = 0, pitch = 0.12;
  let active = 0, activeTarget = 0;
  let reveal = 0, revealTarget = 0;
  // parçacık geçişi (her aç/kapa'da 0->1 patlama)
  let burst = 1, burstActive = false;

  // render the heavy raymarch at reduced internal resolution, CSS upscales.
  const SCALE = 0.9;

  const VERT = `
    void main(){ gl_Position = vec4(position.xy, 0.0, 1.0); }`;

  // ---- Schwarzschild-approx geodesic raymarcher (units: r_s = 1) ----
  // STEPS must be a const int (GLSL ES 1.00 requires constant loop bounds).
  const FRAG = `
    precision highp float;
    uniform vec2  uRes;
    uniform float uTime;
    uniform float uYaw;
    uniform float uPitch;
    uniform float uActive;
    uniform float uReveal;   // 0 = hidden (power-button state), 1 = full black hole

    const int   STEPS  = 300;      // affine-param integration steps
    const float DT     = 0.10;     // base step (adaptively scaled by radius)
    const float RS     = 1.0;      // event horizon radius (pure black)
    const float DIN    = 2.2;      // disk inner radius (hugs the lensed shadow)
    const float DOUT   = 6.0;      // disk outer radius (compact, fits zoomed frame)
    const float ESCAPE = 30.0;     // ray escaped to infinity
    const float PI     = 3.14159265;
    const float ROLL   = 0.2618;   // 15 derece ekran-dik eksen eğimi

    mat3 rotX(float a){ float c=cos(a), s=sin(a); return mat3(1.,0.,0., 0.,c,-s, 0.,s,c); }
    mat3 rotY(float a){ float c=cos(a), s=sin(a); return mat3(c,0.,s, 0.,1.,0., -s,0.,c); }

    // tight single-piece radial mask for the white disk (crisp inner + outer rims)
    float diskMask(float r){
      float inner = smoothstep(DIN, DIN + 0.18, r);   // crisp inner rim (gets lensed up)
      float outer = smoothstep(DOUT, DOUT - 0.35, r); // crisp outer rim, minimal feather
      return inner * outer;
    }

    void main(){
      // aspect-correct, y-normalized screen coords centered at 0
      vec2 uv = (gl_FragCoord.xy * 2.0 - uRes) / uRes.y;
      // 15 derece roll (ekran-dik eksen)
      float cr = cos(ROLL), sr = sin(ROLL);
      uv = mat2(cr, -sr, sr, cr) * uv;

      // orbit camera: rotate a fixed basis by yaw/pitch
      mat3 R = rotY(uYaw) * rotX(uPitch);
      vec3 ro = R * vec3(0.0, 0.0, 18.0);           // camera pulled back -> smaller model in frame
      vec3 rd = R * normalize(vec3(uv, -2.0));      // wider FOV -> more empty space around the hole

      // geodesic integration variables
      vec3  pos = ro;
      vec3  vel = rd;                               // unit direction (kept unit each step)
      // conserved angular momentum^2 from initial conditions
      vec3  cprod = cross(pos, vel);
      float h2 = dot(cprod, cprod);

      vec3  col   = vec3(0.0);   // composited disk radiance (front-to-back)
      float minR  = 100.0;       // closest approach to photon sphere -> crisp ring (no haze)
      float transmit = 1.0;      // remaining transparency (1 = clear, 0 = fully covered)
      float hitHorizon = 0.0;
      vec3  oldpos = pos;

      // disk surface brightness: dim-but-legible when off, full when on.
      // floor raised so orbiting always reveals the lensed disk regardless of state.
      float diskGain = clamp(uActive, 0.0, 1.0);

      for (int i = 0; i < STEPS; i++){
        oldpos = pos;

        // adaptive step: fine near the hole (strong field / thin disk plane),
        // coarse far away. Catches the second equatorial crossing that forms the
        // over-the-top / under-the-bottom lensed arc.
        float r2o = dot(pos, pos);
        float ro_ = sqrt(max(r2o, 1e-6));
        float dt  = DT * clamp(ro_ * 0.35, 0.35, 1.6);

        // advance position, bend velocity toward the hole, renormalize direction.
        pos += vel * dt;
        float r2 = dot(pos, pos);
        float r  = sqrt(max(r2, 1e-6));

        // Schwarzschild null-geodesic fictitious force: -1.5 * h2 * pos / r^5.
        // Base of pow() is guarded positive. Clamp magnitude to bound the
        // near-field blow-up (NaN-safe).
        vec3  acc = -1.5 * h2 * pos / pow(max(r2, 1e-6), 2.5);
        float am  = min(length(acc), 50.0);
        acc = am * normalize(acc + vec3(1e-9));
        vel += acc * dt;
        // null geodesic -> photon speed constant; keep |vel| = 1 so the bend is
        // physically consistent and step-rate independent.
        vel = normalize(vel);

        // event horizon: swallowed -> pure black, opaque, stop
        if (r < RS){
          hitHorizon = 1.0;
          transmit = 0.0;
          break;
        }

        // track closest approach to the photon sphere (~1.5) -> single crisp ring later
        minR = min(minR, r);

        // accretion disk lives in the y = 0 equatorial plane.
        // detect plane crossing by sign change of y between consecutive samples.
        if (oldpos.y * pos.y < 0.0){
          float lambda = oldpos.y / (oldpos.y - pos.y);   // interp factor to y=0
          vec3  hit = mix(oldpos, pos, lambda);           // exact crossing point
          float rr  = length(hit.xz);
          float m   = diskMask(rr);
          if (m > 0.0){
            // SINGLE-PIECE PURE WHITE disk: crisp + opaque, no color/doppler/gradient.
            float a = clamp(m * diskGain, 0.0, 1.0);
            // ACCUMULATE (no break) so the bent ray can cross again ->
            // secondary lensed image arcing over the top / under the bottom.
            col      += vec3(1.0) * a * transmit;
            transmit *= (1.0 - a);
          }
        }

        if (transmit < 0.004) break;   // fully covered, early out
        if (r > ESCAPE) break;          // escaped to infinity -> transparent bg
      }

      // coverage alpha: opaque at horizon (black), otherwise 1 - transmit.
      float a = max(hitHorizon, 1.0 - transmit);
      a = clamp(a, 0.0, 1.0);

      // pure black & white: crisp clamp (no rolloff -> no grey midtones)
      col = clamp(col, 0.0, 1.0);

      // single crisp white photon ring at closest approach to the photon sphere (~1.5),
      // only for rays that did NOT fall into the hole. No volumetric haze.
      float ring = smoothstep(0.16, 0.0, abs(minR - 1.5)) * (1.0 - hitHorizon);
      vec3  outRGB = col * a + vec3(ring);
      float outA   = clamp(max(a, ring), 0.0, 1.0);
      // uReveal: 0 -> tamamen şeffaf (power-button durumu), 1 -> tam kara delik
      gl_FragColor = vec4(outRGB, outA) * uReveal;
    }`;

  // --- power-button -> particles geçiş katmanı (clip-space, kameradan bağımsız) ---
  const PVERT = `
    uniform float uBurst, uAspect;
    attribute float aAngle, aSeed, aRing;
    varying float vA;
    void main(){
      float Rr = (aRing > 0.5) ? 0.46 : mix(0.40, 0.08, aSeed);     // halka / güç ikonu çizgisi
      float a0 = (aRing > 0.5) ? aAngle : (1.5708 + (aSeed - 0.5) * 0.6);
      float e = 1.0 - pow(1.0 - uBurst, 1.6);
      // patlama YOK: çevredeki ışığa doğru içe çekil + hafif spiral, yavaşça sön
      float r   = mix(Rr, 0.16, e);
      float ang = a0 + e * 2.2;
      vec2 pos = vec2(cos(ang), sin(ang)) * r;
      pos.x /= uAspect;
      gl_Position = vec4(pos, 0.0, 1.0);
      gl_PointSize = mix(2.6, 0.5, e) * 2.0;          // küçülerek
      vA = (1.0 - uBurst) * 0.9;                       // yavaşça sön
    }`;
  const PFRAG = `
    precision mediump float;
    varying float vA;
    void main(){
      float d = distance(gl_PointCoord, vec2(0.5));
      float a = smoothstep(0.5, 0.0, d) * vA;
      if (a < 0.01) discard;
      gl_FragColor = vec4(vec3(1.0), a);
    }`;

  // --- ring çevresinde dönen ambient parçacıklar (yoktan oluş -> 3sn dön -> küçül -> yok) ---
  const AVERT = `
    uniform float uTime, uActive, uAspect;
    attribute float aBase, aSeed, aOuter;
    varying float vA;
    void main(){
      float life = fract(uTime / 3.0 + aSeed);            // 3 saniyelik döngü
      float r   = mix(0.30, 0.40, aOuter);                // ring üstü / ring dışı
      float ang = aBase + uTime * 0.45;                   // yavaş dönüş
      vec2 pos = vec2(cos(ang), sin(ang)) * r;
      pos.x /= uAspect;
      float cr = cos(0.2618), sr = sin(0.2618);
      pos = mat2(cr, -sr, sr, cr) * pos;                  // 15 derece roll (kara delikle hizalı)
      gl_Position = vec4(pos, 0.0, 1.0);
      float fade = sin(life * 3.14159265);                // yoktan belir -> sön
      gl_PointSize = (0.6 + 2.4 * fade) * 1.7;            // büyüyüp küçülür
      vA = fade * uActive;
    }`;
  const AFRAG = `
    precision mediump float;
    varying float vA;
    void main(){
      float d = distance(gl_PointCoord, vec2(0.5));
      float a = smoothstep(0.5, 0.0, d) * vA;
      if (a < 0.01) discard;
      gl_FragColor = vec4(vec3(1.0), a);
    }`;

  let prevStatus = app.status;
  $effect(() => {
    const on = app.status === "on" || app.status === "connecting";
    activeTarget = on ? 1 : 0;
    revealTarget = on ? 1 : 0;
    if (app.status !== prevStatus) { burst = 0; burstActive = true; prevStatus = app.status; }
  });

  function size() {
    const w = host.clientWidth || 1, h = host.clientHeight || 1;
    renderer.setSize(w * SCALE, h * SCALE, false);
    renderer.domElement.style.width = w + "px";
    renderer.domElement.style.height = h + "px";
    mat.uniforms.uRes.value.set(w * SCALE, h * SCALE);
    if (pMat) pMat.uniforms.uAspect.value = w / h;
    if (aMat) aMat.uniforms.uAspect.value = w / h;
  }

  function animate() {
    raf = requestAnimationFrame(animate);
    const t = clock.getElapsedTime();

    // lerp orbit toward mouse target, plus a slow auto drift so it's never dead
    const drift = Math.sin(t * 0.12) * 0.35;
    // sağ-sol normal, yukarı-aşağı yarı miktar
    yaw   += ((mx * 0.9 + drift) - yaw) * 0.05;
    pitch += ((0.12 + my * 0.14) - pitch) * 0.05;
    pitch = Math.max(0.04, Math.min(0.42, pitch));
    active += (activeTarget - active) * 0.05;
    reveal += (revealTarget - reveal) * 0.06;
    if (burstActive) { burst += 0.012; if (burst >= 1) { burst = 1; burstActive = false; } } // yavaş

    mat.uniforms.uTime.value = t;
    mat.uniforms.uYaw.value = yaw;
    mat.uniforms.uPitch.value = pitch;
    mat.uniforms.uActive.value = active;
    mat.uniforms.uReveal.value = reveal;

    points.visible = burst < 1.0;
    pMat.uniforms.uBurst.value = burst;
    pMat.uniforms.uTime.value = t;

    aPoints.visible = reveal > 0.05;
    aMat.uniforms.uTime.value = t;
    aMat.uniforms.uActive.value = active;

    renderer.render(scene, camera);
  }

  onMount(() => {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, premultipliedAlpha: true });
    // single resolution control: rely on SCALE via setSize, not devicePixelRatio.
    renderer.setPixelRatio(1);
    renderer.setClearColor(0x000000, 0); // transparent — no box
    host.appendChild(renderer.domElement);

    scene = new THREE.Scene();
    camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

    const geo = new THREE.PlaneGeometry(2, 2);
    mat = new THREE.ShaderMaterial({
      uniforms: {
        uRes:    { value: new THREE.Vector2(1, 1) },
        uTime:   { value: 0 },
        uYaw:    { value: 0 },
        uPitch:  { value: 0.12 },
        uActive: { value: 0 },
        uReveal: { value: 0 },
      },
      vertexShader: VERT,
      fragmentShader: FRAG,
      transparent: true,
      depthTest: false,
      depthWrite: false,
      premultipliedAlpha: true,   // consistent with renderer + shader output
      blending: THREE.NormalBlending,
    });
    scene.add(new THREE.Mesh(geo, mat));

    // power-button -> particle geçiş katmanı
    const NP = 700;
    const pg = new THREE.BufferGeometry();
    const pAng = new Float32Array(NP), pSeed = new Float32Array(NP), pRing = new Float32Array(NP);
    const pdum = new Float32Array(NP * 3);
    for (let i = 0; i < NP; i++) { pAng[i] = Math.random() * Math.PI * 2; pSeed[i] = Math.random(); pRing[i] = i < NP * 0.85 ? 1 : 0; }
    pg.setAttribute("position", new THREE.BufferAttribute(pdum, 3));
    pg.setAttribute("aAngle", new THREE.BufferAttribute(pAng, 1));
    pg.setAttribute("aSeed", new THREE.BufferAttribute(pSeed, 1));
    pg.setAttribute("aRing", new THREE.BufferAttribute(pRing, 1));
    pMat = new THREE.ShaderMaterial({
      uniforms: { uBurst: { value: 1 }, uAspect: { value: 1 }, uTime: { value: 0 } },
      vertexShader: PVERT, fragmentShader: PFRAG,
      transparent: true, depthTest: false, depthWrite: false, blending: THREE.NormalBlending,
    });
    points = new THREE.Points(pg, pMat); points.visible = false; points.renderOrder = 2; scene.add(points);

    // ring çevresinde dönen ambient parçacıklar (ring üstü 10 + ring dışı 10)
    const NA = 20;
    const ag = new THREE.BufferGeometry();
    const aBase = new Float32Array(NA), aSeed = new Float32Array(NA), aOuter = new Float32Array(NA);
    const adum = new Float32Array(NA * 3);
    for (let i = 0; i < NA; i++) { aBase[i] = Math.random() * Math.PI * 2; aSeed[i] = Math.random(); aOuter[i] = i < NA / 2 ? 0 : 1; }
    ag.setAttribute("position", new THREE.BufferAttribute(adum, 3));
    ag.setAttribute("aBase", new THREE.BufferAttribute(aBase, 1));
    ag.setAttribute("aSeed", new THREE.BufferAttribute(aSeed, 1));
    ag.setAttribute("aOuter", new THREE.BufferAttribute(aOuter, 1));
    aMat = new THREE.ShaderMaterial({
      uniforms: { uTime: { value: 0 }, uActive: { value: 0 }, uAspect: { value: 1 } },
      vertexShader: AVERT, fragmentShader: AFRAG,
      transparent: true, depthTest: false, depthWrite: false, blending: THREE.NormalBlending,
    });
    aPoints = new THREE.Points(ag, aMat); aPoints.renderOrder = 3; scene.add(aPoints);

    // seed initial state consistently with the $effect (treat 'connecting' as on)
    activeTarget = (app.status === "on" || app.status === "connecting") ? 1 : 0;
    active = activeTarget;

    size();
    const ro = new ResizeObserver(size); ro.observe(host);

    const onMove = (e: PointerEvent) => {
      const r = host.getBoundingClientRect();
      mx = ((e.clientX - r.left) / r.width - 0.5) * 2;
      my = -((e.clientY - r.top) / r.height - 0.5) * 2;
    };
    host.addEventListener("pointermove", onMove);

    const onVis = () => {
      if (document.hidden) { cancelAnimationFrame(raf); raf = 0; }
      else if (!raf) animate();
    };
    document.addEventListener("visibilitychange", onVis);

    animate();

    return () => {
      cancelAnimationFrame(raf); ro.disconnect();
      host.removeEventListener("pointermove", onMove);
      document.removeEventListener("visibilitychange", onVis);
      geo.dispose(); mat.dispose();
      renderer.dispose();
      // free the GL context promptly and remove the orphaned canvas (HMR/remount leak)
      renderer.forceContextLoss();
      renderer.domElement.remove();
    };
  });
</script>

<button class="bh" bind:this={host} onclick={() => app.toggle()} aria-label="Kara delik · aç/kapat">
  <svg class="power" class:show={app.status === "off"} viewBox="0 0 100 100" aria-hidden="true">
    <circle class="rim" cx="50" cy="50" r="46" />
    <line class="stem" x1="50" y1="27" x2="50" y2="47" />
    <path class="arc" d="M34 37 A20 20 0 1 0 66 37" />
  </svg>
</button>

<style>
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
