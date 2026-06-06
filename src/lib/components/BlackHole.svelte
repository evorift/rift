<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { app } from "$lib/state.svelte";

  let host: HTMLButtonElement;
  let raf = 0;
  let renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.OrthographicCamera;
  let mat: THREE.ShaderMaterial;
  const clock = new THREE.Clock();

  // mouse-driven orbit targets + lerped state
  let mx = 0, my = 0;
  let yaw = 0, pitch = 0.12;
  let active = 0, activeTarget = 0;

  // render the heavy raymarch at reduced internal resolution, CSS upscales.
  // NOTE: resolution is driven ONLY through this SCALE (setPixelRatio(1) below),
  // so there is no double-scaling with devicePixelRatio.
  const SCALE = 0.72;

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

    const int   STEPS  = 300;      // affine-param integration steps
    const float DT     = 0.10;     // base step (adaptively scaled by radius)
    const float RS     = 1.0;      // event horizon radius (pure black)
    const float DIN    = 2.2;      // disk inner radius (hugs the lensed shadow)
    const float DOUT   = 6.0;      // disk outer radius (compact, fits zoomed frame)
    const float ESCAPE = 30.0;     // ray escaped to infinity
    const float PI     = 3.14159265;

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

      // orbit camera: rotate a fixed basis by yaw/pitch
      mat3 R = rotY(uYaw) * rotX(uPitch);
      vec3 ro = R * vec3(0.0, 0.0, 9.0);            // camera position (distance ~9)
      vec3 rd = R * normalize(vec3(uv, -2.9));      // ray dir (focal length)

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
      gl_FragColor = vec4(outRGB, outA);
    }`;

  $effect(() => {
    activeTarget = (app.status === "on" || app.status === "connecting") ? 1 : 0;
  });

  function size() {
    const w = host.clientWidth || 1, h = host.clientHeight || 1;
    renderer.setSize(w * SCALE, h * SCALE, false);
    renderer.domElement.style.width = w + "px";
    renderer.domElement.style.height = h + "px";
    mat.uniforms.uRes.value.set(w * SCALE, h * SCALE);
  }

  function animate() {
    raf = requestAnimationFrame(animate);
    const t = clock.getElapsedTime();

    // lerp orbit toward mouse target, plus a slow auto drift so it's never dead
    const drift = Math.sin(t * 0.12) * 0.35;
    yaw   += ((mx * 0.9 + drift) - yaw) * 0.05;
    pitch += ((0.12 + my * 0.28) - pitch) * 0.05;
    // near edge-on (Interstellar look); small range so the disk stays a thin lensed band
    pitch = Math.max(0.02, Math.min(0.5, pitch));
    active += (activeTarget - active) * 0.05;

    mat.uniforms.uTime.value = t;
    mat.uniforms.uYaw.value = yaw;
    mat.uniforms.uPitch.value = pitch;
    mat.uniforms.uActive.value = active;
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

<button class="bh" bind:this={host} onclick={() => app.toggle()} aria-label="Kara delik · aç/kapat"></button>

<style>
  .bh { width: 100%; height: 100%; border: none; background: transparent; cursor: pointer; padding: 0; display: block; }
  :global(.bh canvas) { display: block; width: 100% !important; height: 100% !important; }
</style>
