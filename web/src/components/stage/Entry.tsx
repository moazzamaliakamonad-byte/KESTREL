"use client";

import { useFrame } from "@react-three/fiber";
import { useMemo, useRef, type ReactNode } from "react";
import * as THREE from "three";
import { shown } from "@/lib/ship/shown";
import { useShip } from "@/lib/ship/store";
import type { EntrySnapshot } from "@/lib/ship/types";
import { ShipModel, type VehiclePose } from "./Vehicles";

/**
 * The entry from orbit, drawn around the ship: a round Earth below it (in
 * metres, Earth-centred, turned so that "up" at the ship is the scene's up
 * and its motion is +x), the thin line of the atmosphere, stars, the ground
 * track guidance predicted, the ship belly first with its heat glow and
 * plasma, and a camera that rides behind it. Everything is procedural; the
 * land and clouds are noise, not real geography. Simulation only.
 */

const R = 6_371_000;

/** The entry being shown, or null (not flying, or past the hand-over). */
export function entryNow(): EntrySnapshot | null {
  const st = useShip.getState();
  if (st.mode !== "flying" && st.mode !== "done") return null;
  return shown()?.entry ?? null;
}

/** Hides its children while the entry is shown (the landing scenery and vehicle). */
export function NotInEntry({ children }: { children: ReactNode }) {
  const g = useRef<THREE.Group>(null);
  useFrame(() => {
    if (g.current) g.current.visible = entryNow() === null;
  });
  return <group ref={g}>{children}</group>;
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

/**
 * Equirectangular textures for a three.js sphere turned z-up (see `Globe`):
 * column u is longitude u·360° − 180°. Land, sea and ice; clouds apart.
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
      let r: number, g: number, bl: number;
      if (polar) [r, g, bl] = [222, 230, 236];
      else if (h > 0.56) {
        // Land: drier toward the low latitudes, a little relief.
        const t = Math.min(1, (h - 0.56) * 6);
        const dry = Math.max(0, 1 - Math.abs(lat) / 0.6) * fbm(x * 4, y * 4, z * 4, 3);
        r = 74 + 70 * dry + 30 * t;
        g = 84 + 40 * dry + 18 * t;
        bl = 58 + 20 * dry + 10 * t;
      } else {
        // Sea: deeper away from the coast.
        const d = Math.min(1, (0.56 - h) * 5);
        r = 14 + 18 * (1 - d);
        g = 46 + 40 * (1 - d);
        bl = 86 + 50 * (1 - d);
      }
      a.data[k] = r;
      a.data[k + 1] = g;
      a.data[k + 2] = bl;
      a.data[k + 3] = 255;
      // Cloud cover as grey levels: an alpha map reads the green channel.
      const c = fbm(x * 3.5 + 11, y * 3.5 + 7, z * 9 + 2, 5);
      const cover = Math.max(0, Math.min(255, (c - 0.55) * 700));
      b.data[k] = b.data[k + 1] = b.data[k + 2] = cover;
      b.data[k + 3] = 255;
    }
  }
  land.getContext("2d")!.putImageData(a, 0, 0);
  sky.getContext("2d")!.putImageData(b, 0, 0);
  const map = new THREE.CanvasTexture(land);
  map.colorSpace = THREE.SRGBColorSpace;
  map.anisotropy = 4;
  const clouds = new THREE.CanvasTexture(sky);
  clouds.colorSpace = THREE.SRGBColorSpace;
  return { map, clouds };
}

/** The thin blue line of the atmosphere at the limb. */
function atmosphereMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    side: THREE.BackSide,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    uniforms: { uColor: { value: new THREE.Color("#4f9bff") } },
    vertexShader: /* glsl */ `
      varying vec3 vN; varying vec3 vV;
      void main() {
        vec4 wp = modelMatrix * vec4(position, 1.0);
        vN = normalize(mat3(modelMatrix) * normal);
        vV = normalize(cameraPosition - wp.xyz);
        gl_Position = projectionMatrix * viewMatrix * wp;
      }`,
    fragmentShader: /* glsl */ `
      uniform vec3 uColor; varying vec3 vN; varying vec3 vV;
      void main() {
        float rim = pow(1.0 - abs(dot(vN, vV)), 6.0);
        gl_FragColor = vec4(uColor * rim * 1.6, rim);
      }`,
  });
}

/** Earth-centred (z north) point of a latitude/longitude (degrees) at radius `r`. */
function eci(latDeg: number, lonDeg: number, r: number): THREE.Vector3 {
  const la = (latDeg * Math.PI) / 180, lo = (lonDeg * Math.PI) / 180;
  return new THREE.Vector3(Math.cos(la) * Math.cos(lo), Math.cos(la) * Math.sin(lo), Math.sin(la)).multiplyScalar(r);
}

const fwd = new THREE.Vector3();
const up = new THREE.Vector3();
const right = new THREE.Vector3();
const basis = new THREE.Matrix4();
const shift = new THREE.Matrix4();

/** The globe, turned so the ship (at the scene origin) has Earth below it and moves along +x. */
function Globe() {
  const group = useRef<THREE.Group>(null);
  const textures = useRef<ReturnType<typeof earthTextures> | null>(null);
  const earth = useRef<THREE.MeshStandardMaterial>(null);
  const cloud = useRef<THREE.MeshStandardMaterial>(null);
  const atmosphere = useMemo(() => atmosphereMaterial(), []);
  const track = useRef<THREE.BufferGeometry>(null);
  const trackKey = useRef<unknown>(null);
  const site = useRef<THREE.Group>(null);

  useFrame(() => {
    const g = group.current;
    const e = entryNow();
    if (!g) return;
    g.visible = e !== null;
    if (!e) return;
    // The textures are made the first time an entry is shown (a fraction of a second, once).
    if (!textures.current && earth.current && cloud.current) {
      textures.current = earthTextures();
      earth.current.map = textures.current.map;
      earth.current.needsUpdate = true;
      cloud.current.alphaMap = textures.current.clouds;
      cloud.current.needsUpdate = true;
    }
    up.set(...e.position).normalize();
    fwd.set(...e.velocity);
    fwd.addScaledVector(up, -fwd.dot(up)).normalize();
    right.crossVectors(fwd, up);
    basis.makeBasis(fwd, up, right).transpose();
    shift.makeTranslation(0, -Math.hypot(...e.position), 0);
    g.matrix.multiplyMatrices(shift, basis);
    g.matrixWorldNeedsUpdate = true;
    // The ground track guidance predicted before the flight (from the prediction).
    const preview = useShip.getState().preview;
    if (track.current && preview?.entry_track && trackKey.current !== preview) {
      trackKey.current = preview;
      const pts = preview.entry_track.map(([la, lo]) => eci(la, lo, R + 200));
      track.current.setFromPoints(pts);
    }
    if (site.current) site.current.position.copy(eci(e.site[0], e.site[1], R + 300));
  });

  return (
    <group ref={group} matrixAutoUpdate={false} visible={false}>
      {/* three.js spheres are y-up; Earth-centred coordinates are z-up. */}
      <group rotation-x={Math.PI / 2}>
        <mesh>
          <sphereGeometry args={[R, 160, 80]} />
          <meshStandardMaterial ref={earth} color="#ffffff" roughness={0.9} metalness={0} />
        </mesh>
        <mesh>
          <sphereGeometry args={[R + 4_000, 128, 64]} />
          <meshStandardMaterial ref={cloud} color="#ffffff" transparent depthWrite={false} roughness={1} />
        </mesh>
        <mesh material={atmosphere}>
          <sphereGeometry args={[R + 90_000, 128, 64]} />
        </mesh>
      </group>
      <line>
        <bufferGeometry ref={track} />
        <lineBasicMaterial color="#8fe9ff" transparent opacity={0.75} depthWrite={false} />
      </line>
      <group ref={site}>
        <mesh>
          <sphereGeometry args={[1_500, 16, 8]} />
          <meshBasicMaterial color="#8fe9ff" />
        </mesh>
      </group>
    </group>
  );
}

/** Stars far away (they ride with the camera). */
function Stars() {
  const points = useRef<THREE.Points>(null);
  const geometry = useMemo(() => {
    const g = new THREE.BufferGeometry();
    const n = 2500;
    const p = new Float32Array(n * 3);
    let s = 7;
    const rnd = () => ((s = (s * 16807) % 2147483647) / 2147483647);
    for (let i = 0; i < n; i++) {
      const u = rnd() * 2 - 1, t = rnd() * Math.PI * 2, r = Math.sqrt(1 - u * u);
      p.set([r * Math.cos(t) * 2e6, u * 2e6, r * Math.sin(t) * 2e6], i * 3);
    }
    g.setAttribute("position", new THREE.BufferAttribute(p, 3));
    return g;
  }, []);
  useFrame(({ camera }) => {
    const pt = points.current;
    if (!pt) return;
    pt.visible = entryNow() !== null;
    pt.position.copy(camera.position);
  });
  return (
    <points ref={points} geometry={geometry} visible={false} frustumCulled={false}>
      <pointsMaterial color="#dfe8ff" size={1.4} sizeAttenuation={false} depthWrite={false} />
    </points>
  );
}

/** The plasma wake: brightest at the belly, fading downstream and toward its edges, orange to violet with heat. */
function plasmaMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    side: THREE.DoubleSide,
    blending: THREE.AdditiveBlending,
    uniforms: { uHeat: { value: 0 }, uTime: { value: 0 } },
    vertexShader: /* glsl */ `
      varying float vAlong; varying vec3 vN; varying vec3 vV;
      void main() {
        vAlong = uv.y;
        vec4 wp = modelMatrix * vec4(position, 1.0);
        vN = normalize(mat3(modelMatrix) * normal);
        vV = normalize(cameraPosition - wp.xyz);
        gl_Position = projectionMatrix * viewMatrix * wp;
      }`,
    fragmentShader: /* glsl */ `
      uniform float uHeat; uniform float uTime;
      varying float vAlong; varying vec3 vN; varying vec3 vV;
      void main() {
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

/** Angle of attack for a Mach number: about 60° hypersonic, broadside (90°) slow, as the model's lift fades. */
function attack(mach: number): number {
  const s = THREE.MathUtils.clamp((mach - 1.5) / 3.5, 0, 1);
  return THREE.MathUtils.degToRad(90 - 30 * s);
}

const f = new THREE.Vector3();
const n = new THREE.Vector3();
const side = new THREE.Vector3();
const nose = new THREE.Vector3();
const belly = new THREE.Vector3();
const third = new THREE.Vector3();
const m4 = new THREE.Matrix4();
const Y = new THREE.Vector3(0, 1, 0);

/** Rotate `v` about unit axis `k` by `a` (Rodrigues). */
function roll(v: THREE.Vector3, k: THREE.Vector3, a: number): THREE.Vector3 {
  const c = Math.cos(a), s = Math.sin(a);
  const cross = new THREE.Vector3().crossVectors(k, v);
  return v.multiplyScalar(c).add(cross.multiplyScalar(s)).addScaledVector(k, k.dot(v) * (1 - c));
}

/** The ship belly first at its angle of attack, rolled by the bank angle; its glow and plasma trail. */
function EntryShip() {
  const root = useRef<THREE.Group>(null);
  const trail = useRef<THREE.Group>(null);
  const plasma = useMemo(() => plasmaMaterial(), []);
  const wake = useRef<THREE.Mesh>(null);
  const pose = useRef<VehiclePose | null>(null);

  useFrame(({ clock }) => {
    const g = root.current;
    const e = entryNow();
    if (!g) return;
    g.visible = e !== null;
    if (!e) {
      pose.current = null;
      return;
    }
    // Scene: x = horizontal motion, y = up; the velocity climbs at the flight-path angle.
    const gamma = THREE.MathUtils.degToRad(e.gamma_deg);
    f.set(Math.cos(gamma), Math.sin(gamma), 0);
    n.copy(Y).addScaledVector(f, -Y.dot(f)).normalize();
    side.crossVectors(f, n);
    const a = attack(e.mach);
    nose.copy(f).multiplyScalar(Math.cos(a)).addScaledVector(n, Math.sin(a));
    belly.copy(f).multiplyScalar(Math.sin(a)).addScaledVector(n, -Math.cos(a));
    const bank = THREE.MathUtils.degToRad(e.bank_deg);
    roll(nose, f, bank);
    roll(belly, f, bank);
    third.crossVectors(belly, nose);
    m4.makeBasis(belly, nose, third);
    g.quaternion.setFromRotationMatrix(m4);
    // Airflow in body axes (body x = model x, body y = −model z, body z = model y).
    const fx = f.dot(belly), fy = f.dot(nose), fz = f.dot(third);
    const heat = THREE.MathUtils.clamp(e.heat_flux / 250_000, 0, 1);
    pose.current = {
      surfaces: [0.55, 0.55, 0.45, 0.45],
      thrust: [0, 0, 0],
      engines: ["off", "off", "off"],
      throttle: 0,
      rcs: [],
      heat,
      flowBody: [fx, -fz, fy],
      vacuum: THREE.MathUtils.clamp(1 - e.density / 1.225, 0, 1),
    };
    // Plasma: a glow on the windward belly and a wake streaming back from it.
    if (trail.current) {
      trail.current.quaternion.setFromUnitVectors(Y, f.clone().negate());
      const flicker = 0.85 + 0.15 * Math.sin(clock.elapsedTime * 23) * Math.sin(clock.elapsedTime * 7.3);
      const u = (wake.current?.material as THREE.ShaderMaterial | undefined)?.uniforms;
      if (!u) return;
      u.uHeat.value = heat * flicker;
      u.uTime.value = clock.elapsedTime;
    }
  });

  return (
    <>
      <group ref={root} visible={false}>
        <ShipModel pose={pose} />
      </group>
      <group ref={trail}>
        {/* The wake: its +y runs downstream from the belly. */}
        <mesh ref={wake} position-y={90} material={plasma}>
          <cylinderGeometry args={[30, 9, 180, 32, 8, true]} />
        </mesh>
      </group>
    </>
  );
}

const camPos = new THREE.Vector3();
const camLook = new THREE.Vector3();

/** Rides behind and above the ship, drifting slowly around it. */
function EntryCamera() {
  const was = useRef(false);
  useFrame(({ camera, clock }, dt) => {
    const e = entryNow();
    if (!e) {
      was.current = false;
      return;
    }
    const gamma = THREE.MathUtils.degToRad(e.gamma_deg);
    f.set(Math.cos(gamma), Math.sin(gamma), 0);
    const a = 0.6 + 0.35 * Math.sin(clock.elapsedTime * 0.05);
    // Beside the ship, a little ahead and above: the plasma streams past.
    camPos.set(0, 0, 0).addScaledVector(f, 40 + 30 * Math.sin(a)).add(new THREE.Vector3(0, 35, 0)).add(new THREE.Vector3(0, 0, 185));
    camLook.copy(f).multiplyScalar(-25);
    const k = was.current ? 1 - Math.exp(-dt * 2) : 1;
    was.current = true;
    camera.position.lerp(camPos, k);
    camera.up.set(0, 1, 0);
    camera.lookAt(camLook);
    const cam = camera as THREE.PerspectiveCamera;
    if (cam.fov !== 40) {
      cam.fov = 40;
      cam.updateProjectionMatrix();
    }
  });
  return null;
}

/** Everything drawn during the entry from orbit. */
export function Entry() {
  return (
    <>
      <Stars />
      <Globe />
      <EntryShip />
      <EntryCamera />
    </>
  );
}
