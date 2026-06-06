<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { app } from "$lib/state.svelte";

  let host: HTMLButtonElement;
  let raf = 0;
  let renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.PerspectiveCamera;
  let pMat: THREE.ShaderMaterial, ring: THREE.Mesh;
  const clock = new THREE.Clock();
  let mx = 0, my = 0, yaw = 0, pitch = 0.5, active = 0, activeTarget = 0;

  const RIN = 1.35, ROUT = 4.2, RS = 1.12, COUNT = 7000;

  const PVERT = `
    uniform float uTime,uState,uPitch,uYaw;
    attribute float aRadius,aAngle,aSpeed,aSeed;
    varying float vA;
    void main(){
      float baseR=mix(${RIN.toFixed(2)},${ROUT.toFixed(2)},aRadius);
      float fall=fract(uTime*0.05*aSpeed + aSeed);     // sürekli içe düşüş
      float r=mix(baseR, ${RIN.toFixed(2)}, fall*uState);
      r += (1.0-uState)*(2.5+aSeed*3.0);               // kapalı: dışa dağıl
      float omega=aSpeed*pow(max(0.4,r),-1.3);
      float ang=aAngle + uTime*omega*1.4;
      vec3 p=vec3(cos(ang)*r, sin(ang)*r, 0.0);
      p.z += sin(ang*2.0+aSeed*6.28)*0.05*r;           // disk kalınlığı
      float cx=cos(uPitch),sx=sin(uPitch),cy=cos(uYaw),sy=sin(uYaw);
      vec3 q=vec3(p.x, p.y*cx-p.z*sx, p.y*sx+p.z*cx);
      q=vec3(q.x*cy+q.z*sy, q.y, -q.x*sy+q.z*cy);
      vec4 mv=modelViewMatrix*vec4(q,1.0);
      gl_PointSize=(1.3+2.2*uState)*(300.0/-mv.z);
      gl_Position=projectionMatrix*mv;
      float fin=smoothstep(${RIN.toFixed(2)}*0.85, ${RIN.toFixed(2)}*1.35, r); // ufukta yutul
      float fout=smoothstep(${ROUT.toFixed(2)}+2.5, ${ROUT.toFixed(2)}-0.3, r); // dışta sön (kutu yok)
      vA=fin*fout*clamp(uState*1.3,0.0,1.0);
    }`;
  const PFRAG = `
    varying float vA;
    void main(){
      float d=distance(gl_PointCoord, vec2(0.5));
      float a=smoothstep(0.5,0.0,d)*vA;
      if(a<0.01) discard;
      gl_FragColor=vec4(vec3(1.0), a);   // saf beyaz
    }`;

  $effect(() => { activeTarget = (app.status === "on" || app.status === "connecting") ? 1 : 0; });

  function size() {
    const w = host.clientWidth || 1, h = host.clientHeight || 1;
    renderer.setSize(w, h);
    camera.aspect = w / h; camera.updateProjectionMatrix();
  }

  function animate() {
    raf = requestAnimationFrame(animate);
    yaw += (mx * 0.8 - yaw) * 0.06;
    pitch += (0.5 + my * 0.5 - pitch) * 0.06;
    active += (activeTarget - active) * 0.05;
    pMat.uniforms.uTime.value = clock.getElapsedTime();
    pMat.uniforms.uState.value = active;
    pMat.uniforms.uPitch.value = pitch;
    pMat.uniforms.uYaw.value = yaw;
    (ring.material as THREE.MeshBasicMaterial).opacity = 0.18 + active * 0.7;
    renderer.render(scene, camera);
  }

  onMount(() => {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(2, window.devicePixelRatio));
    renderer.setClearColor(0x000000, 0); // şeffaf — kutu yok
    host.appendChild(renderer.domElement);

    scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
    camera.position.set(0, 0, 7);

    // parçacıklar
    const g = new THREE.BufferGeometry();
    const aR = new Float32Array(COUNT), aA = new Float32Array(COUNT), aS = new Float32Array(COUNT), aSe = new Float32Array(COUNT);
    const dummy = new Float32Array(COUNT * 3);
    for (let i = 0; i < COUNT; i++) {
      aR[i] = Math.pow(Math.random(), 0.6);
      aA[i] = Math.random() * Math.PI * 2;
      aS[i] = 0.6 + Math.random() * 1.0;
      aSe[i] = Math.random();
    }
    g.setAttribute("position", new THREE.BufferAttribute(dummy, 3));
    g.setAttribute("aRadius", new THREE.BufferAttribute(aR, 1));
    g.setAttribute("aAngle", new THREE.BufferAttribute(aA, 1));
    g.setAttribute("aSpeed", new THREE.BufferAttribute(aS, 1));
    g.setAttribute("aSeed", new THREE.BufferAttribute(aSe, 1));
    pMat = new THREE.ShaderMaterial({
      uniforms: { uTime: { value: 0 }, uState: { value: 0 }, uPitch: { value: 0.5 }, uYaw: { value: 0 } },
      vertexShader: PVERT, fragmentShader: PFRAG,
      transparent: true, depthWrite: false, depthTest: true, blending: THREE.NormalBlending,
    });
    scene.add(new THREE.Points(g, pMat));

    // olay ufku (saf siyah, opak, parçacıkları arkadan kapatır)
    const disc = new THREE.Mesh(new THREE.CircleGeometry(RS, 96), new THREE.MeshBasicMaterial({ color: 0x000000 }));
    scene.add(disc);
    // foton halkası (ince beyaz)
    ring = new THREE.Mesh(new THREE.RingGeometry(RS * 1.0, RS * 1.06, 128),
      new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.2, depthWrite: false, side: THREE.DoubleSide }));
    ring.position.z = 0.01;
    scene.add(ring);

    activeTarget = app.status === "on" ? 1 : 0; active = activeTarget;
    size();
    const ro = new ResizeObserver(size); ro.observe(host);
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
      cancelAnimationFrame(raf); ro.disconnect();
      host.removeEventListener("pointermove", onMove);
      document.removeEventListener("visibilitychange", onVis);
      renderer.dispose();
    };
  });
</script>

<button class="bh" bind:this={host} onclick={() => app.toggle()} aria-label="Kara delik · aç/kapat"></button>

<style>
  .bh { width: 100%; height: 100%; border: none; background: transparent; cursor: pointer; padding: 0; display: block; }
  :global(.bh canvas) { display: block; width: 100% !important; height: 100% !important; }
</style>
