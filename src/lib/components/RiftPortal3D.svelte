<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";

  let { status = "off", onToggle }: { status?: "off" | "connecting" | "on"; onToggle?: () => void } = $props();

  let host: HTMLDivElement;
  let raf = 0;
  let renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.OrthographicCamera, mat: THREE.ShaderMaterial;
  const clock = new THREE.Clock();
  let uStateTarget = 0;
  let mouseX = 0, mouseY = 0;

  const VERT = `varying vec2 vUv; void main(){ vUv=uv; gl_Position=vec4(position.xy,0.0,1.0); }`;

  const FRAG = `
    precision highp float;
    uniform float uTime, uState, uAspect; uniform vec2 uMouse; varying vec2 vUv;

    float hash(vec2 p){ return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
    float noise(vec2 p){ vec2 i=floor(p),f=fract(p); f=f*f*(3.0-2.0*f);
      return mix(mix(hash(i),hash(i+vec2(1,0)),f.x), mix(hash(i+vec2(0,1)),hash(i+vec2(1,1)),f.x), f.y); }
    float fbm(vec2 p){ float a=0.5,s=0.0; for(int i=0;i<5;i++){ s+=a*noise(p); p=p*2.02+1.7; a*=0.5; } return s; }

    float energyAt(vec2 uv){
      float ang=atan(uv.y,uv.x); float r=length(uv);
      vec2 warp=vec2(fbm(uv*3.0+uTime*0.15), fbm(uv*3.0-uTime*0.12+7.0));
      vec2 p=vec2(ang*1.6+uTime*0.25, r*5.0-uTime*0.75)+warp*0.9;
      return fbm(p);
    }

    void main(){
      vec2 uv=(vUv-0.5); uv.x*=uAspect;
      uv += uMouse*0.04*uState;                 // hafif parallaks

      float openW=mix(0.16,0.40,uState);
      float squashX=mix(6.0,1.8,uState);        // dormant: ince dikey yarık
      float d=length(vec2(uv.x*squashX, uv.y/1.18));
      float mask=1.0-smoothstep(openW*0.78, openW, d);

      float e=energyAt(uv); e=pow(e, mix(3.0,1.25,uState));
      float edge=smoothstep(openW*0.5, openW, d);
      float off=edge*0.045;
      float eR=energyAt(uv+vec2(off,0.0)); float eB=energyAt(uv-vec2(off,0.0));

      vec3 cyan=vec3(0.1,0.95,1.3), mag=vec3(1.3,0.2,1.05);
      vec3 col=mix(cyan,mag, clamp(0.5+uv.y*0.85+0.2*sin(uTime+uv.y*8.0),0.0,1.0));
      col.r*=(0.55+0.85*eR); col.g*=(0.55+0.85*e); col.b*=(0.55+0.85*eB);

      float core=e*mask;
      float seam=(1.0-smoothstep(0.0,openW*0.55,d))*mix(0.45,1.0,uState);
      vec3 outc=col*(core*1.9 + seam*1.25);

      float glow=exp(-d*mix(11.0,5.5,uState))*mix(0.12,0.5,uState);
      outc += vec3(0.25,0.75,1.0)*glow;

      float a=clamp(core + seam*0.85 + glow, 0.0, 1.0) * mix(0.55,1.0,uState);
      gl_FragColor=vec4(outc*a, a);             // premultiplied
    }`;

  let prev = status;
  $effect(() => {
    if (status === prev) return;
    uStateTarget = (status === "on" || status === "connecting") ? 1 : 0;
    prev = status;
  });

  function size() {
    const w = host.clientWidth || 1, h = host.clientHeight || 1;
    renderer.setSize(w, h);
    mat.uniforms.uAspect.value = w / h;
  }

  function animate() {
    raf = requestAnimationFrame(animate);
    mat.uniforms.uTime.value = clock.getElapsedTime();
    const cur = mat.uniforms.uState.value;
    mat.uniforms.uState.value += (uStateTarget - cur) * 0.05;
    mat.uniforms.uMouse.value.set(mouseX, mouseY);
    renderer.render(scene, camera);
  }

  onMount(() => {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, premultipliedAlpha: true });
    renderer.setPixelRatio(Math.min(2, window.devicePixelRatio));
    renderer.setClearAlpha(0); // ŞEFFAF — kutu yok
    host.appendChild(renderer.domElement);

    scene = new THREE.Scene();
    camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    mat = new THREE.ShaderMaterial({
      uniforms: {
        uTime: { value: 0 }, uState: { value: status === "on" ? 1 : 0 },
        uAspect: { value: 1 }, uMouse: { value: new THREE.Vector2() },
      },
      vertexShader: VERT, fragmentShader: FRAG,
      transparent: true, depthWrite: false, depthTest: false,
    });
    uStateTarget = status === "on" ? 1 : 0;
    scene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), mat));

    size();
    const ro = new ResizeObserver(size); ro.observe(host);
    const onMove = (e: PointerEvent) => {
      const r = host.getBoundingClientRect();
      mouseX = ((e.clientX - r.left) / r.width - 0.5) * 2;
      mouseY = -((e.clientY - r.top) / r.height - 0.5) * 2;
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

<button class="rift-host" bind:this={host} onclick={onToggle} aria-label="Rift'i aç/kapat"></button>

<style>
  .rift-host {
    position: relative; width: 100%; height: 100%; min-height: 320px;
    border: none; background: transparent; cursor: pointer; padding: 0; display: block;
  }
  :global(.rift-host canvas) { display: block; width: 100% !important; height: 100% !important; }
</style>
