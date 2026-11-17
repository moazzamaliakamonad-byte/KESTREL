"use client";

import { shown as shownState } from "@/lib/ship/shown";
import { Environment } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { entryNow } from "./Entry";
import { useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { useShip } from "@/lib/ship/store";
import type { ScenarioInfo } from "@/lib/ship/types";
import { Backdrop, skin } from "./Blueprint";
import { hash } from "./materials";
import { LaunchSite, MOUNT_TOP } from "./Site";

export { MOUNT_TOP };

/** Direction to the sun (scene axes): mid-morning, over the sea. */
export const SUN = new THREE.Vector3(0.52, 0.42, 0.74).normalize();

const SKY = { zenith: "#1b5aa8", horizon: "#bcd8ee", ground: "#8fb3c9", sun: "#fff3d6", fog: "#b9d3e6" };

/**
 * The sky as seen from the camera's altitude: blue low
 * down, thinning to a bright band on a curved horizon and black with stars
 * as the camera climbs. A scattering look-alike, not a scattering model.
 */
function Sky() {
  const mesh = useRef<THREE.Mesh>(null);
  const material = useMemo(() => {
    const c = SKY;
    return new THREE.ShaderMaterial({
      side: THREE.BackSide,
      depthWrite: false,
      depthTest: false,
      fog: false,
      uniforms: {
        uZenith: { value: new THREE.Color(c.zenith) },
        uHorizon: { value: new THREE.Color(c.horizon) },
        uGround: { value: new THREE.Color(c.ground) },
        uSunColor: { value: new THREE.Color(c.sun) },
        uSun: { value: SUN },
        uAlt: { value: 0 },
        uScale: { value: 8500 },
        uRadius: { value: 6.371e6 },
      },
      vertexShader: /* glsl */ `
        varying vec3 vDir;
        void main() {
          vDir = normalize(position);
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }`,
      fragmentShader: /* glsl */ `
        uniform vec3 uZenith; uniform vec3 uHorizon; uniform vec3 uGround; uniform vec3 uSunColor;
        uniform vec3 uSun; uniform float uAlt; uniform float uScale; uniform float uRadius;
        varying vec3 vDir;
        float hash(vec3 p) { return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453); }
        void main() {
          vec3 d = normalize(vDir);
          // Air left above the camera (1 on the ground, → 0 in space).
          float air = exp(-uAlt / uScale);
          float dip = sqrt(2.0 * uAlt / uRadius);
          float up = (d.y + dip) / (1.0 + dip);
          vec3 zenith = uZenith * pow(air, 0.75);
          // The horizon band gets thinner and sharper with altitude.
          float band = exp(-max(up, 0.0) * mix(3.2, 26.0, 1.0 - air));
          vec3 col = mix(zenith, uHorizon * (0.55 + 0.45 * air), band);
          if (up < 0.0) col = mix(uHorizon * (0.55 + 0.45 * air), uGround, smoothstep(0.0, 0.12, -up));
          float s = max(dot(d, uSun), 0.0);
          col += uSunColor * (pow(s, 3000.0) * 30.0 + pow(s, 180.0) * 0.6 + pow(s, 12.0) * 0.16 * (0.4 + 0.6 * air));
          // Stars where the sky has gone dark.
          vec3 cell = floor(d * 700.0);
          float h = hash(cell);
          float star = step(0.9988, h) * (0.35 + 0.65 * fract(h * 917.0)) * smoothstep(0.35, 0.0, air) * step(0.0, up);
          col += vec3(star);
          gl_FragColor = vec4(col, 1.0);
        }`,
    });
  }, []);
  useFrame(({ camera }) => {
    const m = mesh.current;
    if (!m) return;
    m.position.copy(camera.position);
    (m.material as THREE.ShaderMaterial).uniforms.uAlt.value = Math.max(0, camera.position.y);
  });
  return (
    <mesh ref={mesh} material={material} renderOrder={-10} frustumCulled={false}>
      <sphereGeometry args={[1000, 48, 24]} />
    </mesh>
  );
}

/** What the steel reflects: a small copy of the sky with a bright sun. */
function Reflections() {
  const c = SKY;
  const sun = SUN.clone().multiplyScalar(40);
  return (
    <Environment resolution={128} frames={1} background={false}>
      <mesh scale={100}>
        <sphereGeometry args={[1, 24, 12]} />
        <shaderMaterial
          side={THREE.BackSide}
          uniforms={{ a: { value: new THREE.Color(c.zenith) }, b: { value: new THREE.Color(c.horizon) }, g: { value: new THREE.Color(c.ground) } }}
          vertexShader={`varying vec3 d; void main(){ d = normalize(position); gl_Position = projectionMatrix * modelViewMatrix * vec4(position,1.0); }`}
          fragmentShader={`uniform vec3 a; uniform vec3 b; uniform vec3 g; varying vec3 d; void main(){ vec3 c = d.y > 0.0 ? mix(b, a, pow(d.y, 0.5)) : mix(b, g * 0.6, pow(-d.y, 0.4)); gl_FragColor = vec4(c, 1.0); }`}
        />
      </mesh>
      <mesh position={sun.toArray()}>
        <sphereGeometry args={[5, 16, 8]} />
        <meshBasicMaterial color={[30, 28, 24]} toneMapped={false} />
      </mesh>
    </Environment>
  );
}

/**
 * A few clouds: scattered fair-weather puffs low down and a thin high
 * veil. Kept sparse on purpose (a full deck hid the vehicle and the
 * coast from above), and each layer fades out as the camera passes
 * through its height so it is never a flat sheet across the picture.
 */
function Clouds() {
  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
        fog: false,
        uniforms: { uTime: { value: 0 }, uCover: { value: 0.7 }, uOpacity: { value: 0.55 }, uScale: { value: 3400 } },
        vertexShader: /* glsl */ `
          #include <common>
          #include <logdepthbuf_pars_vertex>
          varying vec3 vWorld;
          void main() {
            vec4 w = modelMatrix * vec4(position, 1.0);
            vWorld = w.xyz;
            gl_Position = projectionMatrix * viewMatrix * w;
            #include <logdepthbuf_vertex>
          }`,
        fragmentShader: /* glsl */ `
          #include <logdepthbuf_pars_fragment>
          varying vec3 vWorld;
          uniform float uTime; uniform float uCover; uniform float uOpacity; uniform float uScale;
          float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
          float noise(vec2 p) {
            vec2 i = floor(p); vec2 f = fract(p); f = f * f * (3.0 - 2.0 * f);
            return mix(mix(hash(i), hash(i + vec2(1, 0)), f.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), f.x), f.y);
          }
          float fbm(vec2 p) { float a = 0.5; float v = 0.0; for (int i = 0; i < 5; i++) { v += a * noise(p); p *= 2.03; a *= 0.5; } return v; }
          void main() {
            #include <logdepthbuf_fragment>
            vec2 p = vWorld.xz / uScale + vec2(uTime * 0.003, 0.0);
            float n = fbm(p) + 0.3 * fbm(p * 3.1);
            float a = smoothstep(uCover, uCover + 0.38, n);
            float edge = smoothstep(60000.0, 18000.0, length(vWorld.xz - cameraPosition.xz));
            // Thin out as the camera passes through the layer, and keep the
            // sky straight above the launch site clear.
            float through = smoothstep(120.0, 1400.0, abs(cameraPosition.y - vWorld.y));
            float site = smoothstep(900.0, 4200.0, length(vWorld.xz));
            float shade = 0.8 + 0.2 * smoothstep(uCover, uCover + 0.5, n);
            gl_FragColor = vec4(vec3(shade) * vec3(1.0, 0.99, 0.97), a * edge * through * site * uOpacity);
          }`,
      }),
    [],
  );
  const upper = useMemo(() => {
    const m = material.clone();
    m.uniforms.uCover = { value: 0.66 };
    m.uniforms.uOpacity = { value: 0.3 };
    m.uniforms.uScale = { value: 9000 };
    return m;
  }, [material]);
  const low = useRef<THREE.Mesh>(null);
  const high = useRef<THREE.Mesh>(null);
  useFrame(({ clock }) => {
    if (low.current) (low.current.material as THREE.ShaderMaterial).uniforms.uTime.value = clock.elapsedTime;
    if (high.current) (high.current.material as THREE.ShaderMaterial).uniforms.uTime.value = clock.elapsedTime * 0.5 + 40;
  });
  return (
    <group>
      <mesh ref={low} rotation-x={-Math.PI / 2} position-y={2300} material={material} renderOrder={2}>
        <planeGeometry args={[120000, 120000]} />
      </mesh>
      <mesh ref={high} rotation-x={-Math.PI / 2} position-y={9500} material={upper} renderOrder={2}>
        <planeGeometry args={[120000, 120000]} />
      </mesh>
    </group>
  );
}

let coastCache: { map: THREE.Texture; rough: THREE.Texture } | null = null;

/** Coast from above: land and sea in one painted texture, the sea glossy. Made once. */
function coastTextures(): { map: THREE.Texture; rough: THREE.Texture } {
  if (coastCache) return coastCache;
  const n = 1024;
  const c = document.createElement("canvas");
  const r = document.createElement("canvas");
  c.width = c.height = r.width = r.height = n;
  const g = c.getContext("2d")!;
  const rg = r.getContext("2d")!;
  const img = g.createImageData(n, n);
  const rimg = rg.createImageData(n, n);
  // Value noise from a table of random numbers (fast: no trigonometry per pixel).
  const T = 256;
  const table = new Float32Array(T * T);
  for (let i = 0; i < table.length; i++) table[i] = hash(i * 0.618 + 3.1);
  const noise = (x: number, y: number) => {
    const xi = Math.floor(x);
    const yi = Math.floor(y);
    let fx = x - xi;
    let fy = y - yi;
    fx = fx * fx * (3 - 2 * fx);
    fy = fy * fy * (3 - 2 * fy);
    const x0 = xi & 255;
    const x1 = (xi + 1) & 255;
    const y0 = (yi & 255) * T;
    const y1 = ((yi + 1) & 255) * T;
    return (table[y0 + x0] * (1 - fx) + table[y0 + x1] * fx) * (1 - fy) + (table[y1 + x0] * (1 - fx) + table[y1 + x1] * fx) * fy;
  };
  const fbm = (x: number, y: number) => noise(x, y) * 0.55 + noise(x * 2.1, y * 2.1) * 0.28 + noise(x * 4.3, y * 4.3) * 0.17;
  // The shore runs across the picture about 700 m beyond the launch site
  // (toward scene +z, the seaward side); it only depends on x.
  const shore = new Float32Array(n);
  for (let x = 0; x < n; x++) shore[x] = 0.506 + 0.008 * (fbm(x / 90, 3.3) - 0.5) + 0.003 * (fbm(x / 14, 9.1) - 0.5);
  for (let y = 0; y < n; y++) {
    const v = y / n;
    for (let x = 0; x < n; x++) {
      const i = 4 * (y * n + x);
      const k = fbm(x / 40, y / 40);
      if (v > shore[x]) {
        const depth = Math.min(1, (v - shore[x]) * 50);
        img.data[i] = 18 + 60 * (1 - depth) + 8 * k;
        img.data[i + 1] = 70 + 70 * (1 - depth) + 10 * k;
        img.data[i + 2] = 110 + 50 * (1 - depth) + 12 * k;
        rimg.data[i] = rimg.data[i + 1] = rimg.data[i + 2] = 40;
      } else {
        const beach = Math.max(0, 1 - (shore[x] - v) * 300);
        const scrub = noise(x / 22, y / 22 + 40) * 0.6 + noise(x / 7, y / 7) * 0.4;
        img.data[i] = 150 + 60 * beach - 50 * scrub + 20 * k;
        img.data[i + 1] = 138 + 50 * beach - 20 * scrub + 16 * k;
        img.data[i + 2] = 104 + 50 * beach - 48 * scrub + 10 * k;
        rimg.data[i] = rimg.data[i + 1] = rimg.data[i + 2] = 240;
      }
      img.data[i + 3] = rimg.data[i + 3] = 255;
    }
  }
  g.putImageData(img, 0, 0);
  rg.putImageData(rimg, 0, 0);
  const map = new THREE.CanvasTexture(c);
  map.colorSpace = THREE.SRGBColorSpace;
  map.anisotropy = 8;
  coastCache = { map, rough: new THREE.CanvasTexture(r) };
  return coastCache;
}

const GROUND_SIZE = 120_000;

function EarthGround() {
  const t = useMemo(() => coastTextures(), []);
  const detail = useMemo(() => {
    const c = document.createElement("canvas");
    c.width = c.height = 256;
    const g = c.getContext("2d")!;
    const img = g.createImageData(256, 256);
    for (let i = 0; i < 256 * 256; i++) {
      const v = 205 + 40 * hash(i * 0.731);
      img.data[4 * i] = v;
      img.data[4 * i + 1] = v * 0.94;
      img.data[4 * i + 2] = v * 0.8;
      img.data[4 * i + 3] = 255;
    }
    g.putImageData(img, 0, 0);
    const tex = new THREE.CanvasTexture(c);
    tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
    tex.repeat.set(160, 160);
    tex.colorSpace = THREE.SRGBColorSpace;
    return tex;
  }, []);
  const feather = useMemo(() => {
    const c = document.createElement("canvas");
    c.width = c.height = 128;
    const g = c.getContext("2d")!;
    const grad = g.createRadialGradient(64, 64, 20, 64, 64, 64);
    grad.addColorStop(0, "#fff");
    grad.addColorStop(0.6, "#ddd");
    grad.addColorStop(1, "#000");
    g.fillStyle = grad;
    g.fillRect(0, 0, 128, 128);
    return new THREE.CanvasTexture(c);
  }, []);
  return (
    <group>
      {/* Beyond the painted coast: land one way, sea the other, out to the horizon. */}
      <mesh rotation-x={-Math.PI / 2} position={[0, -8, -1_000_000 + 700]}>
        <planeGeometry args={[4_000_000, 2_000_000]} />
        <meshStandardMaterial color="#7d7a5c" roughness={1} />
      </mesh>
      <mesh rotation-x={-Math.PI / 2} position={[0, -8, 1_000_000 + 700]}>
        <planeGeometry args={[4_000_000, 2_000_000]} />
        <meshStandardMaterial color="#123f5e" roughness={0.3} envMapIntensity={0.9} />
      </mesh>
      {/* The site sits about 700 m inland of the painted shore. */}
      <mesh rotation-x={-Math.PI / 2} position={[0, -2, 0]} receiveShadow>
        <planeGeometry args={[GROUND_SIZE, GROUND_SIZE]} />
        <meshStandardMaterial map={t.map} roughnessMap={t.rough} roughness={1} metalness={0} envMapIntensity={0.9} />
      </mesh>
      {/* Near-field sand with grain, fading out at its edge, so the ground has detail up close. */}
      <mesh rotation-x={-Math.PI / 2} position={[0, -0.6, -700]} receiveShadow>
        <planeGeometry args={[5200, 5200]} />
        <meshStandardMaterial map={detail} alphaMap={feather} color="#a39375" roughness={1} transparent />
      </mesh>
      <LaunchSite />
    </group>
  );
}

type Catch = NonNullable<ScenarioInfo["catch"]>;
const STEEL = "#34373c";
const SECTION = 9;

function lattice(c: Catch): { p: THREE.Vector3; q: THREE.Quaternion; len: number }[] {
  const h = c.mast_half;
  const out: { p: THREE.Vector3; q: THREE.Quaternion; len: number }[] = [];
  const corners = [
    [-h, -h],
    [h, -h],
    [h, h],
    [-h, h],
  ];
  const up = new THREE.Vector3(0, 1, 0);
  const member = (a: THREE.Vector3, b: THREE.Vector3) => {
    const d = b.clone().sub(a);
    const len = d.length();
    out.push({ p: a.clone().add(b).multiplyScalar(0.5), q: new THREE.Quaternion().setFromUnitVectors(up, d.normalize()), len });
  };
  for (const [x, z] of corners) member(new THREE.Vector3(x, 0, z), new THREE.Vector3(x, c.tower_top, z));
  for (let y = 0; y < c.tower_top; y += SECTION) {
    const y2 = Math.min(y + SECTION, c.tower_top);
    for (let k = 0; k < 4; k++) {
      const [x1, z1] = corners[k];
      const [x2, z2] = corners[(k + 1) % 4];
      member(new THREE.Vector3(x1, y2, z1), new THREE.Vector3(x2, y2, z2));
      const flip = (y / SECTION) % 2 === 0;
      member(new THREE.Vector3(flip ? x1 : x2, y, flip ? z1 : z2), new THREE.Vector3(flip ? x2 : x1, y2, flip ? z2 : z1));
    }
  }
  return out;
}

/**
 * The launch-and-catch tower: a square lattice mast on the −x side of the
 * catch axis and two arms that ride it (at the height of the vehicle being
 * caught) and swing closed. Generic procedural design at about real scale.
 */
function Tower({ c, radius }: { c: Catch; radius: number }) {
  const members = useMemo(() => lattice(c), [c]);
  const mesh = useRef<THREE.InstancedMesh>(null);
  const arms = useRef<(THREE.Group | null)[]>([]);
  const carriage = useRef<THREE.Group>(null);
  const shown = useRef(0);
  const height = useRef(c.arm_z);

  useLayoutEffect(() => {
    const m = mesh.current;
    if (!m) return;
    const mat = new THREE.Matrix4();
    members.forEach((b, i) => {
      mat.compose(b.p, b.q, new THREE.Vector3(1, b.len, 1));
      m.setMatrixAt(i, mat);
    });
    m.instanceMatrix.needsUpdate = true;
  }, [members]);

  const face = c.mast_x + c.mast_half;
  const reach = -face + 13;
  useFrame((_, dt) => {
    const mode = useShip.getState().mode;
    const closed = (mode === "flying" || mode === "done") && !!shownState()?.arms;
    shown.current += ((closed ? 1 : 0) - shown.current) * Math.min(1, dt * 1.4);
    height.current += (c.arm_z - height.current) * Math.min(1, dt * 2);
    if (carriage.current) carriage.current.position.y = height.current;
    const gap = radius + 0.7;
    arms.current.forEach((a, i) => {
      if (!a) return;
      const side = i === 0 ? 1 : -1;
      const closedAngle = Math.atan2(gap - 3.5, reach);
      const openAngle = THREE.MathUtils.degToRad(34);
      a.rotation.y = side * (closedAngle + (openAngle - closedAngle) * (1 - shown.current));
    });
  });

  return (
    <group>
      <group position={[c.mast_x, 0, 0]}>
        <instancedMesh ref={mesh} args={[undefined, undefined, members.length]} castShadow receiveShadow>
          <boxGeometry args={[0.75, 1, 0.75]} />
          <meshStandardMaterial color={STEEL} metalness={0.7} roughness={0.55} />
        </instancedMesh>
        <mesh position-y={c.tower_top + 7}>
          <cylinderGeometry args={[0.15, 0.4, 14, 8]} />
          <meshStandardMaterial color={STEEL} />
        </mesh>
        {/* Quick-disconnect arm for the ship, high on the mast. */}
        <mesh position={[c.mast_half + 5, 120, 4]}>
          <boxGeometry args={[10, 2.2, 2.2]} />
          <meshStandardMaterial color="#44484e" metalness={0.6} roughness={0.6} />
        </mesh>
      </group>
      <group ref={carriage} position-y={c.arm_z}>
        <mesh position={[c.mast_x, 0, 0]} castShadow>
          <boxGeometry args={[c.mast_half * 2 + 2.4, 7, c.mast_half * 2 + 2.4]} />
          <meshStandardMaterial color="#25282c" metalness={0.6} roughness={0.55} />
        </mesh>
        {[0, 1].map((i) => (
          <group key={i} position={[face, 0, (i === 0 ? -1 : 1) * 3.5]} ref={(g) => void (arms.current[i] = g)}>
            {/* A truss arm: two chords and a landing rail on top. */}
            <mesh position={[reach / 2, -1.2, 0]} castShadow>
              <boxGeometry args={[reach, 0.9, 1.5]} />
              <meshStandardMaterial color="#3d4147" metalness={0.65} roughness={0.5} />
            </mesh>
            <mesh position={[reach / 2, 1.0, 0]} castShadow>
              <boxGeometry args={[reach, 0.9, 1.5]} />
              <meshStandardMaterial color="#3d4147" metalness={0.65} roughness={0.5} />
            </mesh>
            {Array.from({ length: 9 }, (_, k) => (
              <mesh key={k} position={[(k + 0.5) * (reach / 9), -0.1, 0]} rotation-z={k % 2 ? 0.75 : -0.75}>
                <boxGeometry args={[0.35, 3.2, 1.1]} />
                <meshStandardMaterial color="#4a4e55" metalness={0.6} roughness={0.55} />
              </mesh>
            ))}
            <mesh position={[reach * 0.55, 1.7, 0]}>
              <boxGeometry args={[reach * 0.8, 0.45, 2.3]} />
              <meshStandardMaterial color="#d09a32" metalness={0.2} roughness={0.7} />
            </mesh>
          </group>
        ))}
      </group>
    </group>
  );
}

/** Haze that thins as the camera climbs. */
function Haze() {
  useFrame(({ camera, scene }) => {
    if (!(scene.fog instanceof THREE.FogExp2)) scene.fog = new THREE.FogExp2(SKY.fog, 0.00003);
    // (No haze over the blueprint.)
    scene.fog.density = skin.on || entryNow() !== null ? 0 : 0.000024 * Math.exp(-Math.max(0, camera.position.y) / 9000) + 0.0000012;
  });
  return null;
}

export function World() {
  const info = useShip((s) => s.info[s.scenario]);
  const sun = SUN;
  const scenery = useRef<THREE.Group>(null);
  // The blueprint view shows the vehicle alone: the world steps aside.
  useFrame(() => {
    // ...and so does it for the entry from orbit (a globe is drawn instead).
    if (scenery.current) scenery.current.visible = !skin.on && entryNow() === null;
  });
  return (
    <group>
      <Reflections />
      <Haze />
      <Backdrop />
      <hemisphereLight args={["#d6e8ff", "#8c7a5e", 0.55]} />
      <directionalLight
        position={sun.clone().multiplyScalar(6000).toArray()}
        intensity={2.5}
        color="#fff1dc"
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-camera-left={-220}
        shadow-camera-right={220}
        shadow-camera-top={220}
        shadow-camera-bottom={-220}
        shadow-camera-near={100}
        shadow-camera-far={12000}
        shadow-bias={-0.0004}
      />
      <group ref={scenery}>
        <Sky />
        <EarthGround />
        <Clouds />
        {info?.catch && <Tower c={info.catch} radius={info.ship.radius} />}
      </group>
    </group>
  );
}
