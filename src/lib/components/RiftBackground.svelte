<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { EffectComposer } from "three/addons/postprocessing/EffectComposer.js";
  import { RenderPass } from "three/addons/postprocessing/RenderPass.js";
  import { UnrealBloomPass } from "three/addons/postprocessing/UnrealBloomPass.js";
  import { app } from "$lib/state.svelte";

  let host: HTMLDivElement;
  let raf = 0;
  let renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.PerspectiveCamera, composer: EffectComposer;
  let rift: THREE.Mesh, mat: THREE.ShaderMaterial;
  const clock = new THREE.Clock();
  let openTarget = 0, open = 0;
  let mx = 0, my = 0, rotX = 0, rotY = 0;

  const VERT = `
    varying vec3 vN; varying vec3 vV; varying vec3 vP;
    void main(){
      vec4 wp = modelMatrix * vec4(position,1.0);
      vP = position;
      vN = normalize(mat3(modelMatrix) * normal);
      vV = normalize(cameraPosition - wp.xyz);
      gl_Position = projectionMatrix * viewMatrix * wp;
    }`;
  const FRAG = `
    uniform float uIntensity, uTime; varying vec3 vN; varying vec3 vV; varying vec3 vP;
    void main(){
      float f = pow(1.0 - clamp(abs(dot(normalize(vN), normalize(vV))), 0.0, 1.0), 2.2);
      vec3 cyan = vec3(0.10, 0.95, 1.25);
      vec3 mag  = vec3(1.30, 0.15, 1.05);
      float h = clamp(vP.y * 0.22 + 0.5, 0.0, 1.0);
      // kenar rengi: alt magenta, üst cyan, hafif zaman kayması
      vec3 rimCol = mix(mag, cyan, h);
      rimCol = mix(rimCol, rimCol.bgr, 0.12 + 0.12 * sin(uTime * 0.6));
      vec3 core = vec3(0.75, 1.0, 1.3) * (1.0 - f);     // içi parlak
      vec3 rim  = rimCol * f * 3.2;                      // kenarlardan cyan/magenta
      vec3 col = (core * 1.25 + rim) * uIntensity;
      gl_FragColor = vec4(col, 1.0);
    }`;

  function buildLens() {
    const H = 2.3, R = 0.95, S = 48;
    const pts: THREE.Vector2[] = [];
    for (let i = 0; i <= S; i++) {
      const t = -Math.PI / 2 + Math.PI * (i / S);
      const y = Math.sin(t) * H;
      const x = Math.pow(Math.max(0, Math.cos(t)), 1.7) * R;
      pts.push(new THREE.Vector2(Math.max(0.0008, x), y));
    }
    const geo = new THREE.LatheGeometry(pts, 80);
    geo.computeVertexNormals();
    return geo;
  }

  $effect(() => { openTarget = (app.status === "on" || app.status === "connecting") ? 1 : 0; });

  function resize() {
    const w = window.innerWidth, h = window.innerHeight;
    renderer.setSize(w, h); composer.setSize(w, h);
    camera.aspect = w / h; camera.updateProjectionMatrix();
  }

  function animate() {
    raf = requestAnimationFrame(animate);
    const t = clock.getElapsedTime();
    open += (openTarget - open) * 0.05;
    mat.uniforms.uTime.value = t;
    mat.uniforms.uIntensity.value = 0.5 + open * 1.1;
    rift.scale.x = 0.12 + open * 0.88;             // dormant: kapalı ince yarık
    rift.scale.z = 0.34;                            // yassı lens
    // fare ile yerinde dönüş (konum sabit)
    rotY += (mx * 0.6 - rotY) * 0.06;
    rotX += (my * 0.35 - rotX) * 0.06;
    rift.rotation.y = rotY + t * 0.05;
    rift.rotation.x = rotX;
    composer.render();
  }

  onMount(() => {
    renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(Math.min(1.5, window.devicePixelRatio));
    renderer.setClearColor(0x04060a, 1);
    host.appendChild(renderer.domElement);

    scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(45, window.innerWidth / window.innerHeight, 0.1, 100);
    camera.position.set(0, 0, 7);

    mat = new THREE.ShaderMaterial({
      uniforms: { uIntensity: { value: 0.5 }, uTime: { value: 0 } },
      vertexShader: VERT, fragmentShader: FRAG, side: THREE.DoubleSide,
    });
    rift = new THREE.Mesh(buildLens(), mat);
    scene.add(rift);

    composer = new EffectComposer(renderer);
    composer.addPass(new RenderPass(scene, camera));
    composer.addPass(new UnrealBloomPass(new THREE.Vector2(window.innerWidth, window.innerHeight), 1.35, 0.7, 0.12));

    open = app.status === "on" ? 1 : 0; openTarget = open;

    const onMove = (e: PointerEvent) => {
      mx = (e.clientX / window.innerWidth - 0.5) * 2;
      my = -(e.clientY / window.innerHeight - 0.5) * 2;
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("resize", resize);
    const onVis = () => { if (document.hidden) { cancelAnimationFrame(raf); raf = 0; } else if (!raf) animate(); };
    document.addEventListener("visibilitychange", onVis);
    animate();

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("resize", resize);
      document.removeEventListener("visibilitychange", onVis);
      renderer.dispose();
    };
  });
</script>

<div class="rift-bg" bind:this={host}></div>

<style>
  .rift-bg { position: fixed; inset: 0; z-index: 0; pointer-events: none; }
  :global(.rift-bg canvas) { display: block; }
</style>
