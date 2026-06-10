<script>
  // Uygulamadaki BlackHole.svelte'in DEKORATİF kopyası (orijinale dokunulmadı).
  // Fark: `app` state bağımlılığı yok — her zaman "açık" (reveal=1), tıklayınca onactivate().
  // Shader'lar (kara delik + parçacıklar) birebir aynıdır.
  import { onMount } from "svelte";
  import * as THREE from "three";

  let { onactivate } = $props();

  let host;
  let raf = 0;
  let renderer, scene, camera;
  let mat;
  let pMat, points;
  const clock = new THREE.Clock();

  let mx = 0, my = 0;
  let yaw = 0, pitch = 0.25;
  let active = 0;
  const activeTarget = 1; // landing: her zaman açık

  // sabitlenmiş ayarlar
  const rollDeg = -15;
  const ringR = 8.9;
  const blackR = 5.7;
  const yawDeg = 47;
  const pitchBaseDeg = 15;
  const pitchUpDeg = 13;
  const pitchDnDeg = 4;

  const SCALE = 0.9;

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

      float rdr = abs(minR - 1.5);
      float ringCore = smoothstep(0.16, 0.0, rdr);
      float ringGlow = smoothstep(0.55, 0.0, rdr) * 0.22;
      float ring = (ringCore + ringGlow) * (1.0 - hitHorizon);
      vec3  outRGB = col * a + vec3(ring);
      float outA   = clamp(max(a, ring), 0.0, 1.0);
      gl_FragColor = vec4(outRGB, outA) * uReveal;
    }`;

  const PROJ = `
    uniform float uYaw, uPitch, uAspect, uRoll;
    const float CAMD = 18.0;
    mat3 rotX(float a){ float c=cos(a), s=sin(a); return mat3(1.,0.,0., 0.,c,-s, 0.,s,c); }
    mat3 rotY(float a){ float c=cos(a), s=sin(a); return mat3(c,0.,s, 0.,1.,0., -s,0.,c); }
    vec4 project(vec3 P){
      vec3 Pc = rotX(-uPitch) * rotY(-uYaw) * P;
      vec2 uv = 2.0 * Pc.xy / (CAMD - Pc.z);
      float cr = cos(uRoll), sr = sin(uRoll);
      uv = mat2(cr, sr, -sr, cr) * uv;
      uv.x /= uAspect;
      return vec4(uv, 0.0, 1.0);
    }`;

  const PART_VERT = PROJ + `
    uniform float uTime, uActive, uRing, uBlackR, uLife;
    attribute float aSeed, aIndex, aBlack;
    varying float vA, vBlack;
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
      float sr = length(2.0 * Pc.xy / depth);
      float behind = smoothstep(0.05, -0.05, Pc.z);
      float inSil  = smoothstep(0.56, 0.368, sr);
      float occ = behind * inSil;
      float grow = sin(life * 3.14159265);
      gl_PointSize = (0.4 + 3.6 * grow) * 1.7;
      vA = grow * smoothstep(0.0, 0.2, uActive) * (1.0 - occ);
      vBlack = aBlack;
    }`;
  const PART_FRAG = `
    precision mediump float;
    varying float vA, vBlack;
    void main(){
      float d = distance(gl_PointCoord, vec2(0.5));
      float core = smoothstep(0.42, 0.0, d);
      float halo = smoothstep(0.5, 0.0, d) * 0.35;
      float a = (core + halo) * vA;
      if (a < 0.01) discard;
      vec3 col = mix(vec3(1.0), vec3(0.0), vBlack);
      gl_FragColor = vec4(col, a);
    }`;

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
    const drift = Math.sin(t * 0.12) * 0.06;
    const D2R = Math.PI / 180;
    const yawT = mx * (yawDeg * D2R) + drift;
    const pitchT = (pitchBaseDeg * D2R) + (my >= 0 ? my * pitchUpDeg : my * pitchDnDeg) * D2R;
    yaw += (yawT - yaw) * 0.06;
    pitch += (pitchT - pitch) * 0.06;
    pitch = Math.max(0.0, Math.min(1.35, pitch));

    active += (activeTarget - active) * 0.05;
    const reveal = Math.max(0, Math.min(1, (active - 0.3) / 0.7));

    const rollRad = (rollDeg * Math.PI) / 180;
    mat.uniforms.uTime.value = t;
    mat.uniforms.uYaw.value = yaw;
    mat.uniforms.uPitch.value = pitch;
    mat.uniforms.uActive.value = reveal;
    mat.uniforms.uReveal.value = reveal;
    mat.uniforms.uRoll.value = rollRad;

    pMat.uniforms.uTime.value = t;
    pMat.uniforms.uActive.value = active;
    pMat.uniforms.uYaw.value = yaw;
    pMat.uniforms.uPitch.value = pitch;
    pMat.uniforms.uRoll.value = rollRad;
    pMat.uniforms.uRing.value = ringR;
    pMat.uniforms.uBlackR.value = blackR;
    points.visible = active > 0.001;

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

    const N = 198;
    const pg = new THREE.BufferGeometry();
    const aSeed = new Float32Array(N), aIndex = new Float32Array(N), aBlack = new Float32Array(N);
    const dum = new Float32Array(N * 3);
    for (let i = 0; i < N; i++) {
      aSeed[i] = Math.random();
      aIndex[i] = i + 1;
      aBlack[i] = i >= N - 30 ? 1 : 0;
    }
    pg.setAttribute("position", new THREE.BufferAttribute(dum, 3));
    pg.setAttribute("aSeed", new THREE.BufferAttribute(aSeed, 1));
    pg.setAttribute("aIndex", new THREE.BufferAttribute(aIndex, 1));
    pg.setAttribute("aBlack", new THREE.BufferAttribute(aBlack, 1));
    pMat = new THREE.ShaderMaterial({
      uniforms: {
        uTime: { value: 0 }, uActive: { value: 0 }, uAspect: { value: 1 }, uLife: { value: 3.5 },
        uYaw: { value: 0 }, uPitch: { value: 0.25 }, uRoll: { value: 0 },
        uRing: { value: 8.9 }, uBlackR: { value: 5.7 },
      },
      vertexShader: PART_VERT, fragmentShader: PART_FRAG,
      transparent: true, depthTest: false, depthWrite: false, blending: THREE.NormalBlending,
    });
    points = new THREE.Points(pg, pMat); points.visible = false; points.renderOrder = 2; scene.add(points);

    size();
    const obs = new ResizeObserver(size); obs.observe(host);
    const onMove = (e) => {
      const r = host.getBoundingClientRect();
      mx = ((e.clientX - r.left) / r.width - 0.5) * 2;
      my = -((e.clientY - r.top) / r.height - 0.5) * 2;
    };
    host.addEventListener("pointermove", onMove);
    const onLeave = () => { mx = 0; my = 0; };
    host.addEventListener("pointerleave", onLeave);
    const onVis = () => { if (document.hidden) { cancelAnimationFrame(raf); raf = 0; } else if (!raf) animate(); };
    document.addEventListener("visibilitychange", onVis);
    animate();

    return () => {
      cancelAnimationFrame(raf); obs.disconnect();
      host.removeEventListener("pointermove", onMove);
      host.removeEventListener("pointerleave", onLeave);
      document.removeEventListener("visibilitychange", onVis);
      geo.dispose(); mat.dispose(); pg.dispose(); pMat.dispose();
      renderer.dispose(); renderer.forceContextLoss(); renderer.domElement.remove();
    };
  });
</script>

<div class="bh-wrap">
  <button class="bh" bind:this={host} onclick={() => onactivate?.()} aria-label="Rift"></button>
</div>

<style>
  .bh-wrap { position: relative; width: 100%; height: 100%; }
  .bh { position: relative; width: 100%; height: 100%; border: none; background: transparent; cursor: pointer; padding: 0; display: block; }
  :global(.bh canvas) { position: absolute; inset: 0; z-index: 1; display: block; width: 100% !important; height: 100% !important; }
</style>
