<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { app } from "$lib/state.svelte";

  let host: HTMLButtonElement;
  let raf = 0;
  let renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.OrthographicCamera, mat: THREE.ShaderMaterial;
  const clock = new THREE.Clock();
  let mx = 0, my = 0, yaw = 0, pitch = 0.35, active = 0, activeTarget = 0;

  const VERT = `varying vec2 vUv; void main(){ vUv=uv; gl_Position=vec4(position.xy,0.0,1.0); }`;

  const FRAG = `
    precision highp float;
    uniform vec2 uRes; uniform float uTime, uYaw, uPitch, uActive;
    varying vec2 vUv;
    float hash(vec2 p){ return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5); }
    float noise(vec2 p){ vec2 i=floor(p),f=fract(p); f=f*f*(3.0-2.0*f);
      return mix(mix(hash(i),hash(i+vec2(1,0)),f.x),mix(hash(i+vec2(0,1)),hash(i+vec2(1,1)),f.x),f.y); }

    void main(){
      vec2 ndc=(vUv-0.5)*2.0; float aspect=uRes.x/uRes.y; ndc.x*=aspect;

      float cp=cos(uPitch), sp=sin(uPitch), cy=cos(uYaw), sy=sin(uYaw);
      float D=9.0;
      vec3 cam=vec3(D*cy*cp, D*sp, D*sy*cp);
      vec3 fwd=normalize(-cam);
      vec3 rgt=normalize(cross(vec3(0,1,0),fwd));
      vec3 up=cross(fwd,rgt);
      vec3 rd=normalize(fwd + ndc.x*rgt*0.5 + ndc.y*up*0.5);

      vec3 pos=cam, vel=rd;
      vec3 hv=cross(pos,vel); float h2=dot(hv,hv);

      vec3 col=vec3(0.0); float alpha=0.0;
      float rin=2.4, rout=6.6, dt=0.18, prevY=pos.y;
      for(int i=0;i<150;i++){
        vec3 acc=-1.5*h2*pos/pow(dot(pos,pos),2.5);   // gravitasyonel ışık bükülmesi
        vel+=acc*dt;
        vec3 np=pos+vel*dt;
        float r=length(np);
        if(r<1.0){ col=vec3(0.0); alpha=1.0; break; }   // olay ufku (siyah)
        if(prevY*np.y<0.0){                               // disk düzlemi geçişi
          float t=prevY/(prevY-np.y);
          vec3 cr=mix(pos,np,t);
          float rc=length(cr.xz);
          if(rc>rin && rc<rout){
            float temp=smoothstep(rout,rin,rc);
            float ang=atan(cr.z,cr.x);
            float band=0.55+0.45*noise(vec2(ang*3.0+uTime*0.5, rc*1.6-uTime*1.6));
            vec3 orb=normalize(vec3(-cr.z,0.0,cr.x));
            float dop=0.55+0.75*clamp(dot(orb,normalize(cam-cr)),0.0,1.0); // doppler
            vec3 hot=mix(vec3(1.0,0.45,1.1), vec3(0.45,1.0,1.45), temp);
            hot=mix(hot, vec3(1.3,1.2,1.5), temp*temp*0.6);
            float br=temp*band*dop*(0.18+0.95*uActive);
            col+=hot*br; alpha=max(alpha, clamp(br,0.0,1.0));
          }
        }
        prevY=np.y; pos=np;
        if(r>40.0) break;
      }
      gl_FragColor=vec4(col, alpha);
    }`;

  $effect(() => { activeTarget = (app.status === "on" || app.status === "connecting") ? 1 : 0; });

  function size() {
    const w = host.clientWidth || 1, h = host.clientHeight || 1;
    renderer.setSize(w, h);
    mat.uniforms.uRes.value.set(w, h);
  }

  function animate() {
    raf = requestAnimationFrame(animate);
    yaw += (mx * 0.9 - yaw) * 0.06;
    pitch += (0.35 + my * 0.5 - pitch) * 0.06;
    active += (activeTarget - active) * 0.05;
    mat.uniforms.uTime.value = clock.getElapsedTime();
    mat.uniforms.uYaw.value = yaw;
    mat.uniforms.uPitch.value = pitch;
    mat.uniforms.uActive.value = active;
    renderer.render(scene, camera);
  }

  onMount(() => {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, premultipliedAlpha: false });
    renderer.setPixelRatio(Math.min(1.4, window.devicePixelRatio));
    renderer.setClearAlpha(0);
    host.appendChild(renderer.domElement);
    scene = new THREE.Scene();
    camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    mat = new THREE.ShaderMaterial({
      uniforms: { uRes: { value: new THREE.Vector2(1, 1) }, uTime: { value: 0 }, uYaw: { value: 0 }, uPitch: { value: 0.35 }, uActive: { value: 0 } },
      vertexShader: VERT, fragmentShader: FRAG, transparent: true, depthTest: false, depthWrite: false,
    });
    activeTarget = app.status === "on" ? 1 : 0; active = activeTarget;
    scene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), mat));
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
