"use client";

import { useFrame } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import { toScene, type Vec3 } from "@/lib/ship/frames";
import { shown } from "@/lib/ship/shown";
import { useShip } from "@/lib/ship/store";
import type { EntrySnapshot, ShipSnapshot } from "@/lib/ship/types";

/**
 * The round Earth for a flight from orbit, in the same world as the
 * landing: the tower's frame (scene origin at the site, its up the scene's
 * +y), so the ship flies one continuous path from 120 km to the arms. The
 * globe's top sits just under the flat landing ground; a regional cap
 * around the site continues the painted coast for a thousand kilometres
 * and blends into the globe beyond. Also the atmosphere's thin line at the
 * limb and the plasma wake behind the ship while it is hot. Everything is
 * procedural; land and clouds are noise, not real geography. Simulation only.
 */

export const EARTH_RADIUS = 6_371_000;
/** The scene position of Earth's centre (the site is at the origin, up +y). */
export const EARTH_CENTRE = new THREE.Vector3(0, -EARTH_RADIUS, 0);
/** The globe lies this far under the flat ground at the site, m (no z-fighting). */
const SINK = 150;
/** Radius of the regional cap around the site, m. */
const CAP = 1_200_000;

/** The entry being shown, or null (not flying, or past the hand-over). */
export function entryNow(): EntrySnapshot | null {
  const st = useShip.getState();
  if (st.mode !== "flying" && st.mode !== "done") return null;
  return shown()?.entry ?? null;
}

/** The flight shown is one from orbit (during the entry and after it). */
export function orbitFlight(): ShipSnapshot | null {
  const st = useShip.getState();
  const s = st.mode === "flying" || st.mode === "done" ? shown() : null;
  if (s?.earth_frame) return s;
  return null;
}

/** Altitude above the round Earth of a scene point, m. */
export function trueAltitude(p: THREE.Vector3): number {
  return p.distanceTo(EARTH_CENTRE) - EARTH_RADIUS;
}

// Value noise on the unit sphere (seamless), a few octaves.
function hash3(x: number, y: number, z: number): number {
  const s = Math.sin(x * 127.1 + y * 311.7 + z * 74.7) * 43758.5453;
  return s - Math.floor(s);
}
function noise3(x: number, y: number, z: number): number {
  const xi = Math.floor(x), yi = Math.floor(y), zi = Math.floor(z);
  const xf = x - xi, yf = y - yi, zf = z - zi;
  const u = xf * xf * (3 - 2 * xf), v = yf * yf * (3 - 2 * yf), w = zf * zf * (3 - 2 * zf);
  const l = (a: number, b: number, t: number) => a + (b - a) * t;
  const c = (dx: number, dy: number, dz: number) => hash3(xi + dx, yi + dy, zi + dz);
  return l(
    l(l(c(0, 0, 0), c(1, 0, 0), u), l(c(0, 1, 0), c(1, 1, 0), u), v),
    l(l(c(0, 0, 1), c(1, 0, 1), u), l(c(0, 1, 1), c(1, 1, 1), u), v),
    w,
  );
}
function fbm(x: number, y: number, z: number, octaves: number): number {
  let a = 0.5, f = 1, s = 0;
  for (let i = 0; i < octaves; i++) {
    s += a * noise3(x * f, y * f, z * f);
    a *= 0.5;
    f *= 2.1;
  }
  return s;
}

/** Land and sea colours close to the painted coast's (World.tsx). */
function landColour(dry: number, t: number): [number, number, number] {
  return [118 + 40 * dry + 20 * t, 112 + 26 * dry + 14 * t, 84 + 18 * dry + 8 * t];
}
function seaColour(depth: number): [number, number, number] {
  return [18 + 50 * (1 - depth), 66 + 60 * (1 - depth), 104 + 46 * (1 - depth)];
}

/**
 * Equirectangular textures for a three.js sphere turned z-up (see `Globe`):
 * column u is longitude u·360° − 180° of the globe's own coordinates. Land,
 * sea and ice; clouds as grey levels (an alpha map reads green).
 */
function earthTextures(): { map: THREE.CanvasTexture; clouds: THREE.CanvasTexture } {
  const W = 1024, H = 512;
  const land = document.createElement("canvas");
  land.width = W;
  land.height = H;
  const sky = document.createElement("canvas");
  sky.width = W;
  sky.height = H;
  const a = land.getContext("2d")!.createImageData(W, H);
  const b = sky.getContext("2d")!.createImageData(W, H);
  for (let j = 0; j < H; j++) {
    const lat = Math.PI / 2 - (j / H) * Math.PI;
    for (let i = 0; i < W; i++) {
      const lon = (i / W) * 2 * Math.PI - Math.PI;
      const x = Math.cos(lat) * Math.cos(lon), y = Math.cos(lat) * Math.sin(lon), z = Math.sin(lat);
      const h = fbm(x * 2.2 + 5, y * 2.2 + 3, z * 2.2 + 1, 6);
      const k = (j * W + i) * 4;
      const polar = Math.abs(lat) > 1.2 + 0.08 * (fbm(x * 6, y * 6, z * 6, 2) - 0.5);
      let c: [number, number, number];
      if (polar) c = [222, 230, 236];
      else if (h > 0.56) c = landColour(Math.max(0, 1 - Math.abs(lat) / 0.6) * fbm(x * 4, y * 4, z * 4, 3), Math.min(1, (h - 0.56) * 6));
      else c = seaColour(Math.min(1, (0.56 - h) * 5));
      a.data.set([c[0], c[1], c[2], 255], k);
      const cl = fbm(x * 3.5 + 11, y * 3.5 + 7, z * 9 + 2, 5);
      const cover = Math.max(0, Math.min(255, (cl - 0.55) * 700));
      b.data.set([cover, cover, cover, 255], k);
    }
  }
  land.getContext("2d")!.putImageData(a, 0, 0);
  sky.getContext("2d")!.putImageData(b, 0, 0);
  const map = new THREE.CanvasTexture(land);
  map.colorSpace = THREE.SRGBColorSpace;
  map.anisotropy = 4;
  const clouds = new THREE.CanvasTexture(sky);
  return { map, clouds };
}

/**
 * The region around the site, seen from the scene's top (x across, z
 * down the picture): the painted coast's shore (along x, 700 m toward +z)
 * continues, wandering more the farther it gets; inland and offshore
 * vary; the edge fades into the globe.
 */
function capTextures(): { map: THREE.CanvasTexture; alpha: THREE.CanvasTexture } {
  const N = 1024;
  const c = document.createElement("canvas");
  c.width = c.height = N;
  const al = document.createElement("canvas");
  al.width = al.height = N;
  const img = c.getContext("2d")!.createImageData(N, N);
  const am = al.getContext("2d")!.createImageData(N, N);
  for (let j = 0; j < N; j++) {
    const z = ((j + 0.5) / N - 0.5) * 2 * CAP;
    for (let i = 0; i < N; i++) {
      const x = ((i + 0.5) / N - 0.5) * 2 * CAP;
      const r = Math.hypot(x, z);
      // The shore: straight at the site, wandering with distance.
      const wander = Math.min(1, Math.abs(x) / 400_000);
      const shore = 700 + wander * 260_000 * (fbm(x / 300_000 + 3, 1.7, 0.4, 4) - 0.5) * 2;
      const k = (j * N + i) * 4;
      const n = fbm(x / 60_000 + 9, z / 60_000 + 2, 0.3, 4);
      const col = z < shore ? landColour(0.5 + 0.5 * fbm(x / 150_000, z / 150_000, 5.1, 3), n) : seaColour(Math.min(1, (z - shore) / 120_000));
      img.data.set([col[0], col[1], col[2], 255], k);
      const fade = Math.max(0, Math.min(1, (CAP - r) / (0.35 * CAP)));
      const v = Math.round(255 * fade);
      am.data.set([v, v, v, 255], k);
    }
  }
  c.getContext("2d")!.putImageData(img, 0, 0);
  al.getContext("2d")!.putImageData(am, 0, 0);
  const map = new THREE.CanvasTexture(c);
  map.colorSpace = THREE.SRGBColorSpace;
  map.anisotropy = 8;
  return { map, alpha: new THREE.CanvasTexture(al) };
}

/** A disc of the round Earth around the site, in scene coordinates (site at the origin, up +y). */
function capGeometry(): THREE.BufferGeometry {
  const rings = 64, segs = 128;
  const pos: number[] = [], uv: number[] = [], idx: number[] = [];
  const R = EARTH_RADIUS - SINK / 3;
  for (let r = 0; r <= rings; r++) {
    // Denser near the site.
    const rho = CAP * Math.pow(r / rings, 1.6);
    const th = rho / R;
    for (let s = 0; s <= segs; s++) {
      const ph = (s / segs) * Math.PI * 2;
      const x = R * Math.sin(th) * Math.cos(ph), z = R * Math.sin(th) * Math.sin(ph);
      pos.push(x, R * Math.cos(th) - EARTH_RADIUS, z);
      uv.push(0.5 + x / (2 * CAP), 0.5 - z / (2 * CAP));
    }
  }
  for (let r = 0; r < rings; r++) {
    for (let s = 0; s < segs; s++) {
      const a = r * (segs + 1) + s, b = a + segs + 1;
      idx.push(a, a + 1, b, b, a + 1, b + 1);
    }
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute("uv", new THREE.Float32BufferAttribute(uv, 2));
  g.setIndex(idx);
  g.computeVertexNormals();
  return g;
}

/** The thin blue line of the atmosphere at the limb. */
function atmosphereMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    side: THREE.BackSide,
    transparent: true,
    depthWrite: false,
    fog: false,
    blending: THREE.AdditiveBlending,
    uniforms: { uColor: { value: new THREE.Color("#4f9bff") } },
    vertexShader: /* glsl */ `
      #include <common>
      #include <logdepthbuf_pars_vertex>
      varying vec3 vN; varying vec3 vV;
      void main() {
        vec4 wp = modelMatrix * vec4(position, 1.0);
        vN = normalize(mat3(modelMatrix) * normal);
        vV = normalize(cameraPosition - wp.xyz);
        gl_Position = projectionMatrix * viewMatrix * wp;
        #include <logdepthbuf_vertex>
      }`,
    fragmentShader: /* glsl */ `
      #include <logdepthbuf_pars_fragment>
      uniform vec3 uColor; varying vec3 vN; varying vec3 vV;
      void main() {
        #include <logdepthbuf_fragment>
        float rim = pow(1.0 - abs(dot(vN, vV)), 6.0);
        gl_FragColor = vec4(uColor * rim * 1.6, rim);
      }`,
  });
}

const ex = new THREE.Vector3();
const ey = new THREE.Vector3();
const ez = new THREE.Vector3();
const basis = new THREE.Matrix4();
const shift = new THREE.Matrix4();

/**
 * Earth-centred coordinates → scene: the tower frame's axes (x, y, up =
 * site) mapped to the scene's (x, −z, y), the site at the origin.
 */
function earthToScene(frame: { site: Vec3; x: Vec3; y: Vec3 }, out: THREE.Matrix4): THREE.Matrix4 {
  ex.set(...frame.x);
  ey.set(...frame.site);
  ez.set(...frame.y).negate();
  basis.makeBasis(ex, ey, ez).transpose();
  shift.makeTranslation(0, -EARTH_RADIUS, 0);
  return out.multiplyMatrices(shift, basis);
}

/** The globe, the regional cap and the atmosphere, for a flight from orbit. */
function Globe() {
  const group = useRef<THREE.Group>(null);
  const local = useRef<THREE.Group>(null);
  const textures = useRef<{ earth: ReturnType<typeof earthTextures>; cap: ReturnType<typeof capTextures> } | null>(null);
  const earth = useRef<THREE.MeshStandardMaterial>(null);
  const cloud = useRef<THREE.MeshStandardMaterial>(null);
  const cap = useRef<THREE.MeshStandardMaterial>(null);
  const atmosphere = useMemo(() => atmosphereMaterial(), []);
  const capGeo = useMemo(() => capGeometry(), []);

  useFrame(() => {
    const g = group.current;
    const s = orbitFlight();
    const visible = s !== null;
    if (g) g.visible = visible;
    if (local.current) local.current.visible = visible;
    if (!g || !s?.earth_frame) return;
    // Made the first time a flight from orbit is shown (about a second, once).
    if (!textures.current && earth.current && cloud.current && cap.current) {
      textures.current = { earth: earthTextures(), cap: capTextures() };
      earth.current.map = textures.current.earth.map;
      earth.current.needsUpdate = true;
      cloud.current.alphaMap = textures.current.earth.clouds;
      cloud.current.needsUpdate = true;
      cap.current.map = textures.current.cap.map;
      cap.current.alphaMap = textures.current.cap.alpha;
      cap.current.needsUpdate = true;
    }
    earthToScene(s.earth_frame, g.matrix);
    g.matrixWorldNeedsUpdate = true;
  });

  return (
    <>
      <group ref={group} matrixAutoUpdate={false} visible={false}>
        {/* three.js spheres are y-up; Earth-centred coordinates are z-up. */}
        <group rotation-x={Math.PI / 2}>
          <mesh>
            <sphereGeometry args={[EARTH_RADIUS - SINK, 192, 96]} />
            <meshStandardMaterial ref={earth} color="#ffffff" roughness={0.9} metalness={0} fog={false} />
          </mesh>
          <mesh>
            <sphereGeometry args={[EARTH_RADIUS + 6_000, 128, 64]} />
            <meshStandardMaterial ref={cloud} color="#ffffff" transparent opacity={0.6} depthWrite={false} roughness={1} fog={false} />
          </mesh>
          <mesh material={atmosphere}>
            <sphereGeometry args={[EARTH_RADIUS + 90_000, 128, 64]} />
          </mesh>
        </group>
      </group>
      {/* The region around the site, already in scene coordinates. */}
      <group ref={local} visible={false}>
        <mesh geometry={capGeo} renderOrder={-1}>
          <meshStandardMaterial ref={cap} color="#ffffff" transparent depthWrite={false} roughness={0.95} fog={false} />
        </mesh>
      </group>
    </>
  );
}

/** The plasma wake: brightest at the belly, fading downstream and toward its edges, orange to violet with heat. */
function plasmaMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    fog: false,
    side: THREE.DoubleSide,
    blending: THREE.AdditiveBlending,
    uniforms: { uHeat: { value: 0 }, uTime: { value: 0 } },
    vertexShader: /* glsl */ `
      #include <common>
      #include <logdepthbuf_pars_vertex>
      varying float vAlong; varying vec3 vN; varying vec3 vV;
      void main() {
        vAlong = uv.y;
        vec4 wp = modelMatrix * vec4(position, 1.0);
        vN = normalize(mat3(modelMatrix) * normal);
        vV = normalize(cameraPosition - wp.xyz);
        gl_Position = projectionMatrix * viewMatrix * wp;
        #include <logdepthbuf_vertex>
      }`,
    fragmentShader: /* glsl */ `
      #include <logdepthbuf_pars_fragment>
      uniform float uHeat; uniform float uTime;
      varying float vAlong; varying vec3 vN; varying vec3 vV;
      void main() {
        #include <logdepthbuf_fragment>
        // uv.y is 0 at the narrow (belly) end of the cylinder, 1 downstream.
        float near = pow(1.0 - vAlong, 2.2);
        float edge = pow(abs(dot(vN, vV)), 1.5);
        float ripple = 0.8 + 0.2 * sin(vAlong * 30.0 - uTime * 40.0);
        vec3 hot = mix(vec3(1.0, 0.45, 0.15), vec3(0.85, 0.35, 1.0), clamp(uHeat * 1.2 - 0.3, 0.0, 1.0));
        float a = uHeat * near * (0.35 + edge) * ripple * 1.6;
        gl_FragColor = vec4(hot * a, a);
      }`,
  });
}

const Y = new THREE.Vector3(0, 1, 0);
const flow = new THREE.Vector3();

/** Heat flux (W/m²) at which the plasma is fully bright; below 20 kW/m² there is none. */
const PLASMA_FULL = 250_000;
const PLASMA_START = 20_000;

/** The plasma streaming back from the vehicle while it is hot (any flight). */
function Plasma() {
  const g = useRef<THREE.Group>(null);
  const wake = useRef<THREE.Mesh>(null);
  const material = useMemo(() => plasmaMaterial(), []);
  useFrame(({ clock }) => {
    const group = g.current;
    const st = useShip.getState();
    const s = st.mode === "flying" || st.mode === "done" ? shown() : null;
    const heat = s ? THREE.MathUtils.clamp((s.heat_flux - PLASMA_START) / (PLASMA_FULL - PLASMA_START), 0, 1) : 0;
    if (!group) return;
    group.visible = heat > 0.01 && !st.view.analysis;
    if (!s || !group.visible) return;
    group.position.set(...toScene(s.position));
    flow.set(...toScene(s.velocity)).normalize().negate();
    group.quaternion.setFromUnitVectors(Y, flow);
    const u = (wake.current?.material as THREE.ShaderMaterial | undefined)?.uniforms;
    if (!u) return;
    const flicker = 0.85 + 0.15 * Math.sin(clock.elapsedTime * 23) * Math.sin(clock.elapsedTime * 7.3);
    u.uHeat.value = heat * flicker;
    u.uTime.value = clock.elapsedTime;
  });
  return (
    <group ref={g} visible={false}>
      {/* Its +y runs downstream from the belly. */}
      <mesh ref={wake} position-y={90} material={material}>
        <cylinderGeometry args={[30, 9, 180, 32, 8, true]} />
      </mesh>
    </group>
  );
}

/** Everything a flight from orbit adds to the world. */
export function Entry() {
  return (
    <>
      <Globe />
      <Plasma />
    </>
  );
}
