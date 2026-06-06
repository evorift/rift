<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/addons/controls/OrbitControls.js";
  import { EffectComposer } from "three/addons/postprocessing/EffectComposer.js";
  import { RenderPass } from "three/addons/postprocessing/RenderPass.js";
  import { UnrealBloomPass } from "three/addons/postprocessing/UnrealBloomPass.js";

  let { status = "off", onToggle }: { status?: "off" | "connecting" | "on"; onToggle?: () => void } = $props();

  let host: HTMLDivElement;
  let raf = 0;
  let phase: "closed" | "burst" | "suck" | "open" = "closed";
  let riftTarget = 0, riftScale = 0.0001;
  let timers: number[] = [];
  const clearTimers = () => { timers.forEach(clearTimeout); timers = []; };

  const COUNT = 1500;
  let pos: Float32Array, vel: Float32Array, dead: Uint8Array;
  let pGeo: THREE.BufferGeometry, points: THREE.Points;
  let riftGroup: THREE.Group, riftMat: THREE.ShaderMaterial;
  let composer: EffectComposer, controls: OrbitControls, camera: THREE.PerspectiveCamera, renderer: THREE.WebGLRenderer;
  const clock = new THREE.Clock();

  const FRAG = `
    uniform float uTime; varying vec2 vUv;
    float hash(vec2 p){ return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453123); }
    float noise(vec2 p){ vec2 i=floor(p),f=fract(p); float a=hash(i),b=hash(i+vec2(1.,0.)),c=hash(i+vec2(0.,1.)),d=hash(i+vec2(1.,1.)); vec2 u=f*f*(3.-2.*f); return mix(mix(a,b,u.x),mix(c,d,u.x),u.y); }
    void main(){
      vec2 uv=vUv; float x=uv.x-0.5, y=uv.y-0.5;
      float taper=smoothstep(0.5,0.08,abs(y));
      float width=0.035+0.10*pow(1.0-taper,1.5);
      float wob=(noise(vec2(uv.y*8.0,uTime*1.5))-0.5)*0.06*taper;
      float horiz=abs(x-wob);
      float core=smoothstep(width,0.0,horiz)*taper;
      float halo=smoothstep(0.33,0.0,horiz)*taper;
      float flick=0.75+0.25*noise(vec2(uv.y*30.0,uTime*10.0));
      vec3 cyan=vec3(0.0,0.95,1.0), mag=vec3(1.0,0.15,0.95);
      vec3 col=mix(cyan,mag, clamp(smoothstep(-0.5,0.5,y)+0.15*sin(uTime*1.5+uv.y*10.0),0.0,1.0));
      vec3 outc=col*(core*2.4*flick + halo*0.5);
      gl_FragColor=vec4(outc, max(core, halo*0.5));
    }`;
  const VERT = `varying vec2 vUv; void main(){ vUv=uv; gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }`;

  function spriteTex() {
    const c = document.createElement("canvas"); c.width = c.height = 64;
    const g = c.getContext("2d")!;
    const grd = g.createRadialGradient(32, 32, 0, 32, 32, 32);
    grd.addColorStop(0, "rgba(255,255,255,1)"); grd.addColorStop(0.4, "rgba(255,255,255,0.55)"); grd.addColorStop(1, "rgba(255,255,255,0)");
    g.fillStyle = grd; g.fillRect(0, 0, 64, 64);
    return new THREE.CanvasTexture(c);
  }

  function spawn() {
    for (let i = 0; i < COUNT; i++) {
      dead[i] = 0;
      pos[i * 3] = (Math.random() - 0.5) * 0.2;
      pos[i * 3 + 1] = (Math.random() - 0.5) * 0.2;
      pos[i * 3 + 2] = (Math.random() - 0.5) * 0.2;
      const a = Math.random() * Math.PI * 2, el = (Math.random() - 0.5) * 0.6;
      const s = 2.5 + Math.random() * 4.5;
      vel[i * 3] = Math.cos(a) * s;
      vel[i * 3 + 1] = Math.sin(a) * s;
      vel[i * 3 + 2] = el * s * 0.4;
    }
    pGeo.attributes.position.needsUpdate = true;
  }
  function hideParticles() {
    for (let i = 0; i < COUNT; i++) { dead[i] = 1; pos[i * 3 + 2] = -1000; }
    pGeo.attributes.position.needsUpdate = true;
  }

  let prev = status;
  $effect(() => {
    if (status === prev) return;
    if ((status === "connecting" || status === "on") && prev === "off") { phase = "burst"; spawn(); }
    if (status === "on") {
      clearTimers();
      phase = "suck"; riftTarget = 1;
      timers.push(setTimeout(() => { phase = "open"; }, 1300) as unknown as number);
    }
    if (status === "off") { clearTimers(); phase = "closed"; riftTarget = 0; hideParticles(); }
    prev = status;
  });

  function updateParticles(dt: number) {
    if (phase !== "burst" && phase !== "suck") return;
    for (let i = 0; i < COUNT; i++) {
      if (dead[i]) continue;
      const ix = i * 3, iy = ix + 1, iz = ix + 2;
      if (phase === "burst") {
        const damp = Math.max(0, 1 - 1.4 * dt);
        vel[ix] *= damp; vel[iy] *= damp; vel[iz] *= damp;
      } else {
        const dx = -pos[ix], dy = -pos[iy], dz = -pos[iz];
        const d = Math.hypot(dx, dy, dz) || 1;
        const acc = 14 + (4 - Math.min(4, d)) * 8;
        vel[ix] += (dx / d) * acc * dt;
        vel[iy] += (dy / d) * acc * dt;
        vel[iz] += (dz / d) * acc * dt;
        // spiral
        vel[ix] += (-dy / d) * 6 * dt;
        vel[iy] += (dx / d) * 6 * dt;
        const damp = Math.max(0, 1 - 2.0 * dt);
        vel[ix] *= damp; vel[iy] *= damp; vel[iz] *= damp;
        if (d < 0.3) { dead[i] = 1; pos[iz] = -1000; continue; }
      }
      pos[ix] += vel[ix] * dt; pos[iy] += vel[iy] * dt; pos[iz] += vel[iz] * dt;
    }
    pGeo.attributes.position.needsUpdate = true;
  }

  function animate() {
    raf = requestAnimationFrame(animate);
    const dt = Math.min(0.05, clock.getDelta());
    const t = clock.elapsedTime;
    riftMat.uniforms.uTime.value = t;
    riftScale += (riftTarget - riftScale) * Math.min(1, dt * 5);
    riftGroup.scale.setScalar(Math.max(0.0001, riftScale));
    riftGroup.visible = riftScale > 0.02;
    riftGroup.rotation.y += dt * 0.25;
    updateParticles(dt);
    points.visible = phase === "burst" || phase === "suck";
    controls.update();
    composer.render();
  }

  function resize() {
    const w = host.clientWidth, h = host.clientHeight;
    if (!w || !h) return;
    renderer.setSize(w, h); composer.setSize(w, h);
    camera.aspect = w / h; camera.updateProjectionMatrix();
  }

  onMount(() => {
    const w = host.clientWidth || 600, h = host.clientHeight || 360;
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(2, window.devicePixelRatio));
    renderer.setSize(w, h);
    renderer.setClearColor(0x04060a, 1);
    host.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(50, w / h, 0.1, 100);
    camera.position.set(0, 0, 6);

    // rift: çapraz düzlemler (3D hacim hissi)
    riftMat = new THREE.ShaderMaterial({
      uniforms: { uTime: { value: 0 } }, vertexShader: VERT, fragmentShader: FRAG,
      transparent: true, blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.DoubleSide,
    });
    riftGroup = new THREE.Group();
    for (let i = 0; i < 3; i++) {
      const m = new THREE.Mesh(new THREE.PlaneGeometry(2.6, 4.6), riftMat);
      m.rotation.y = (i * Math.PI) / 3;
      riftGroup.add(m);
    }
    riftGroup.scale.setScalar(0.0001);
    scene.add(riftGroup);

    // parçacıklar
    pos = new Float32Array(COUNT * 3); vel = new Float32Array(COUNT * 3); dead = new Uint8Array(COUNT);
    const col = new Float32Array(COUNT * 3);
    for (let i = 0; i < COUNT; i++) {
      dead[i] = 1; pos[i * 3 + 2] = -1000;
      const mag = Math.random() > 0.5;
      col[i * 3] = mag ? 1.0 : 0.0; col[i * 3 + 1] = mag ? 0.15 : 0.95; col[i * 3 + 2] = mag ? 0.95 : 1.0;
    }
    pGeo = new THREE.BufferGeometry();
    pGeo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    pGeo.setAttribute("color", new THREE.BufferAttribute(col, 3));
    const pMat = new THREE.PointsMaterial({ size: 0.08, map: spriteTex(), vertexColors: true, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false, sizeAttenuation: true });
    points = new THREE.Points(pGeo, pMat); points.visible = false; scene.add(points);

    controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true; controls.dampingFactor = 0.08;
    controls.enablePan = false; controls.enableZoom = true;
    controls.minDistance = 3.5; controls.maxDistance = 9; controls.rotateSpeed = 0.6;

    composer = new EffectComposer(renderer);
    composer.addPass(new RenderPass(scene, camera));
    const bloom = new UnrealBloomPass(new THREE.Vector2(w, h), 1.25, 0.6, 0.0);
    composer.addPass(bloom);

    if (status === "on") { phase = "open"; riftTarget = 1; riftScale = 1; }

    const ro = new ResizeObserver(resize); ro.observe(host);
    const onVis = () => { if (document.hidden) { cancelAnimationFrame(raf); raf = 0; } else if (!raf) animate(); };
    document.addEventListener("visibilitychange", onVis);
    animate();

    return () => {
      cancelAnimationFrame(raf); clearTimers(); ro.disconnect();
      document.removeEventListener("visibilitychange", onVis);
      renderer.dispose();
    };
  });
</script>

<div class="rift-host" bind:this={host}>
  {#if status === "off"}
    <button class="closed-btn" onclick={onToggle} aria-label="Rift'i aç">
      <span class="slit"></span>
      <span class="lbl">RİFT KAPALI · AÇMAK İÇİN TIKLA</span>
    </button>
  {/if}
</div>

<style>
  .rift-host { position: relative; width: 100%; height: 100%; min-height: 320px; }
  :global(.rift-host canvas) { display: block; border-radius: 14px; }

  .closed-btn {
    position: absolute; inset: 0; margin: auto;
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 18px;
    background: transparent; border: none; cursor: pointer; color: var(--text-dim);
  }
  .slit {
    width: 4px; height: 120px; border-radius: 4px;
    background: linear-gradient(180deg, transparent, #6b6fae 20%, #8a6fd6 50%, #6b6fae 80%, transparent);
    box-shadow: 0 0 18px 2px rgba(140,110,230,.5);
    transition: transform .2s;
  }
  .closed-btn:hover .slit { transform: scaleY(1.08); box-shadow: 0 0 26px 3px rgba(150,120,255,.7); }
  .lbl { font-size: 11px; font-weight: 700; letter-spacing: 2px; }
</style>
