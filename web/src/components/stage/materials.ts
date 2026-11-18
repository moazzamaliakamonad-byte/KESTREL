/**
 * Procedural textures and shader materials shared by the vehicles and the
 * world. Everything is generated in the browser: no image files, no
 * third-party assets (D-010).
 */

import * as THREE from "three";

/** Deterministic hash → [0, 1). */
export function hash(n: number): number {
  const s = Math.sin(n * 127.1 + 311.7) * 43758.5453;
  return s - Math.floor(s);
}

function canvas(w: number, h: number): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  return [c, c.getContext("2d")!];
}

function texture(c: HTMLCanvasElement, repeat: [number, number], srgb = true): THREE.CanvasTexture {
  const t = new THREE.CanvasTexture(c);
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.repeat.set(...repeat);
  t.anisotropy = 8;
  if (srgb) t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

/**
 * Rolled stainless rings: one texture tile is one ring (welds top and
 * bottom), with a little brushed variation and heat tint. `rings` = how
 * many rings the mesh's height spans.
 */
export function steelTextures(rings: number): { map: THREE.Texture; roughnessMap: THREE.Texture } {
  const [c, g] = canvas(512, 128);
  const [r, rg] = canvas(512, 128);
  g.fillStyle = "#c9ccd0";
  g.fillRect(0, 0, 512, 128);
  rg.fillStyle = "#5a5a5a";
  rg.fillRect(0, 0, 512, 128);
  for (let x = 0; x < 512; x++) {
    // Vertical brushed streaks and panel seams.
    const k = hash(x * 0.37);
    g.fillStyle = `rgba(${k > 0.5 ? 255 : 40}, ${k > 0.5 ? 255 : 44}, ${k > 0.5 ? 255 : 52}, ${0.015 + 0.035 * hash(x * 1.7)})`;
    g.fillRect(x, 0, 1, 128);
    rg.fillStyle = `rgba(${k > 0.5 ? 150 : 40}, 0, 0, ${0.25 * hash(x * 2.3)})`;
    rg.fillRect(x, 0, 1, 128);
  }
  for (let i = 0; i < 8; i++) {
    g.fillStyle = "rgba(60, 64, 72, 0.25)";
    g.fillRect(i * 64 + Math.floor(hash(i) * 20), 0, 1, 128);
  }
  // Weld lines with a faint straw-coloured heat tint beside them.
  const tint = g.createLinearGradient(0, 0, 0, 128);
  tint.addColorStop(0, "rgba(120, 104, 88, 0.3)");
  tint.addColorStop(0.07, "rgba(150, 138, 122, 0.1)");
  tint.addColorStop(0.2, "rgba(0, 0, 0, 0)");
  tint.addColorStop(0.8, "rgba(0, 0, 0, 0)");
  tint.addColorStop(0.93, "rgba(150, 138, 122, 0.1)");
  tint.addColorStop(1, "rgba(120, 104, 88, 0.3)");
  g.fillStyle = tint;
  g.fillRect(0, 0, 512, 128);
  g.fillStyle = "rgba(60, 62, 68, 0.55)";
  g.fillRect(0, 0, 512, 2);
  rg.fillStyle = "rgba(230, 0, 0, 0.6)";
  rg.fillRect(0, 0, 512, 5);
  return { map: texture(c, [3, rings]), roughnessMap: texture(r, [3, rings], false) };
}

/** Black hexagonal heat-shield tiles with the odd lighter or missing one. */
export function tileTexture(repeat: [number, number]): THREE.Texture {
  const [c, g] = canvas(512, 512);
  g.fillStyle = "#0b0b0d";
  g.fillRect(0, 0, 512, 512);
  const s = 16;
  const h = s * Math.sqrt(3);
  let n = 0;
  for (let row = -1; row < 512 / h + 1; row++) {
    for (let col = -1; col < 512 / (1.5 * s) + 1; col++) {
      const cx = col * 1.5 * s;
      const cy = row * h + (col % 2 ? h / 2 : 0);
      const k = hash(n++);
      const v = 14 + Math.floor(k * 14) + (k > 0.985 ? 60 : 0);
      g.fillStyle = `rgb(${v}, ${v}, ${v + 2})`;
      g.strokeStyle = "#2b2b30";
      g.lineWidth = 1.5;
      g.beginPath();
      for (let i = 0; i < 6; i++) {
        const a = (Math.PI / 3) * i;
        g.lineTo(cx + s * Math.cos(a), cy + s * Math.sin(a));
      }
      g.closePath();
      g.fill();
      g.stroke();
    }
  }
  return texture(c, repeat);
}

/** A see-through lattice for the grid fins (alpha-tested). */
export function gridTexture(): THREE.Texture {
  const [c, g] = canvas(256, 256);
  g.clearRect(0, 0, 256, 256);
  g.strokeStyle = "#8a8d92";
  g.lineWidth = 7;
  for (let i = -8; i <= 16; i++) {
    g.beginPath();
    g.moveTo(i * 32, 0);
    g.lineTo(i * 32 + 256, 256);
    g.stroke();
    g.beginPath();
    g.moveTo(i * 32, 256);
    g.lineTo(i * 32 + 256, 0);
    g.stroke();
  }
  g.lineWidth = 14;
  g.strokeRect(0, 0, 256, 256);
  return texture(c, [1, 1]);
}

/** Soft round puff for smoke and dust sprites. */
export function puffTexture(): THREE.Texture {
  const [c, g] = canvas(128, 128);
  const grad = g.createRadialGradient(64, 64, 2, 64, 64, 64);
  grad.addColorStop(0, "rgba(255,255,255,1)");
  grad.addColorStop(0.35, "rgba(255,255,255,0.55)");
  grad.addColorStop(1, "rgba(255,255,255,0)");
  g.fillStyle = grad;
  g.fillRect(0, 0, 128, 128);
  for (let i = 0; i < 60; i++) {
    g.fillStyle = `rgba(255,255,255,${0.04 * hash(i)})`;
    g.beginPath();
    g.arc(64 + 44 * (hash(i * 3.1) - 0.5) * 2, 64 + 44 * (hash(i * 7.7) - 0.5) * 2, 6 + 14 * hash(i * 1.3), 0, 7);
    g.fill();
  }
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

/**
 * Methane–oxygen exhaust: a blue-white core with shock diamonds near the
 * nozzle and a violet-to-orange fade. The mesh is a unit cone from the
 * nozzle exit (uv.y = 1) to the tip (uv.y = 0), scaled in length.
 */
export function plumeMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.DoubleSide,
    uniforms: {
      uTime: { value: 0 },
      uThrottle: { value: 1 },
      /** 0 = sea level (narrow, hard diamonds), 1 = near vacuum (wide, soft). */
      uVacuum: { value: 0 },
      uSeed: { value: 0 },
    },
    vertexShader: /* glsl */ `
      #include <common>
      #include <logdepthbuf_pars_vertex>
      varying vec2 vUv;
      varying vec3 vNormal;
      varying vec3 vView;
      uniform float uVacuum;
      void main() {
        vUv = uv;
        vec3 p = position;
        // The jet spreads as the air thins.
        float s = 1.0 - uv.y;
        p.xz *= 1.0 + uVacuum * (1.5 + 5.0 * s);
        vec4 mv = modelViewMatrix * vec4(p, 1.0);
        vNormal = normalize(normalMatrix * normal);
        vView = normalize(-mv.xyz);
        gl_Position = projectionMatrix * mv;
        #include <logdepthbuf_vertex>
      }`,
    fragmentShader: /* glsl */ `
      #include <logdepthbuf_pars_fragment>
      varying vec2 vUv;
      varying vec3 vNormal;
      varying vec3 vView;
      uniform float uTime;
      uniform float uThrottle;
      uniform float uVacuum;
      uniform float uSeed;
      void main() {
        #include <logdepthbuf_fragment>
        float s = 1.0 - vUv.y;
        float facing = abs(dot(normalize(vNormal), vView));
        float body = pow(1.0 - s, 1.6);
        float flicker = 0.9 + 0.1 * sin(uTime * 47.0 + uSeed * 9.0 + s * 20.0);
        // Shock diamonds: bright knots that fade down the jet.
        float knots = pow(max(0.0, sin(s * 34.0 - 0.6)), 6.0) * exp(-s * 5.0) * (1.0 - uVacuum);
        vec3 core = vec3(0.75, 0.86, 1.0);
        vec3 mid = vec3(0.45, 0.42, 1.0);
        vec3 tail = vec3(1.0, 0.5, 0.2);
        vec3 col = mix(core, mid, smoothstep(0.0, 0.45, s));
        col = mix(col, tail, smoothstep(0.35, 1.0, s));
        col += knots * vec3(1.0, 0.95, 0.85) * 1.6;
        float a = body * flicker * (0.25 + 0.75 * pow(facing, 0.7)) * (0.35 + 0.65 * uThrottle);
        a *= mix(1.0, 0.45, uVacuum);
        // Additive blending multiplies by alpha once more: keep the colour hot.
        gl_FragColor = vec4(col * 2.4, a);
      }`,
  });
}

/**
 * Entry heating glow: a shell around the body that lights up only on the
 * side facing the airflow. `uHeat` 0–1 from the simulated heating estimate.
 */
export function heatMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.FrontSide,
    uniforms: {
      uHeat: { value: 0 },
      uTime: { value: 0 },
      /** Where the air comes from, in the mesh's local axes (unit). */
      uFlow: { value: new THREE.Vector3(0, -1, 0) },
    },
    vertexShader: /* glsl */ `
      #include <common>
      #include <logdepthbuf_pars_vertex>
      varying vec3 vLocalNormal;
      varying vec3 vNormal;
      varying vec3 vView;
      varying vec3 vPos;
      void main() {
        vLocalNormal = normal;
        vPos = position;
        vec4 mv = modelViewMatrix * vec4(position, 1.0);
        vNormal = normalize(normalMatrix * normal);
        vView = normalize(-mv.xyz);
        gl_Position = projectionMatrix * mv;
        #include <logdepthbuf_vertex>
      }`,
    fragmentShader: /* glsl */ `
      #include <logdepthbuf_pars_fragment>
      varying vec3 vLocalNormal;
      varying vec3 vNormal;
      varying vec3 vView;
      varying vec3 vPos;
      uniform float uHeat;
      uniform float uTime;
      uniform vec3 uFlow;
      void main() {
        #include <logdepthbuf_fragment>
        float windward = max(dot(normalize(vLocalNormal), uFlow), 0.0);
        float rim = pow(1.0 - abs(dot(normalize(vNormal), vView)), 1.5);
        float streak = 0.75 + 0.25 * sin(dot(vPos, uFlow) * 1.7 - uTime * 30.0 + vPos.x * 3.0);
        float glow = (pow(windward, 1.3) * 1.2 + rim * 0.5 * windward) * streak;
        // Dull red → orange → pink-white as the heating grows.
        vec3 cool = vec3(1.0, 0.25, 0.05);
        vec3 hot = vec3(1.0, 0.72, 0.85);
        vec3 col = mix(cool, hot, smoothstep(0.2, 1.0, uHeat));
        float a = glow * uHeat;
        gl_FragColor = vec4(col * a * 2.2, a);
      }`,
  });
}
