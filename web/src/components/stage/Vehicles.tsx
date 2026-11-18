"use client";

import { useFrame } from "@react-three/fiber";
import { useMemo, useRef, type RefObject } from "react";
import * as THREE from "three";
import type { Vec3 } from "@/lib/ship/frames";
import { engineLayout } from "@/lib/ship/engines";
import type { EngineState } from "@/lib/ship/types";
import { Part, skin } from "./Blueprint";
import { gridTexture, hash, heatMaterial, plumeMaterial, steelTextures, tileTexture } from "./materials";

/**
 * The two vehicles, drawn procedurally at real scale from the published
 * outer dimensions: a Starship-class ship (52 m × 9 m) and a Super
 * Heavy-class booster (72.3 m × 9 m, 33 engines, three grid fins). Generic
 * original geometry: no logos, no third-party assets (D-010).
 *
 * Model axes: local +Y is the body axis toward the nose (physics body z),
 * local +X the ship's belly (body x), local −Z body +y. The group origin
 * is the centre of mass.
 */

/** What a model needs to pose itself each frame. `null` = parked, engines cold. */
export interface VehiclePose {
  /** Ship: flap exposures 0–1 (fore L, fore R, aft L, aft R). Booster: fin deflections, rad. */
  surfaces: number[];
  /** Body-frame thrust, N. */
  thrust: Vec3;
  engines: EngineState[];
  /** Mean throttle of the running engines, 0–1. */
  throttle: number;
  rcs: boolean[];
  /** Entry heating, 0–1. */
  heat: number;
  /** Where the air comes from, body axes (unit). */
  flowBody: Vec3;
  /** 0 at sea level, 1 in near vacuum. */
  vacuum: number;
}

export type PoseRef = RefObject<VehiclePose | null>;

const Y = new THREE.Vector3(0, 1, 0);
const tmp = new THREE.Vector3();

/** Body axes → model axes. */
const local = (v: Vec3, out: THREE.Vector3) => out.set(v[0], v[2], -v[1]);

function Bell({ exit, throat, height }: { exit: number; throat: number; height: number }) {
  return (
    <mesh position-y={-height / 2} castShadow>
      <cylinderGeometry args={[throat, exit, height, 24, 1, true]} />
      <meshStandardMaterial color="#4a4541" metalness={0.9} roughness={0.38} side={THREE.DoubleSide} />
    </mesh>
  );
}

/** One engine's exhaust: an outer jet and a hot inner core. */
function Plume({ exit, plumeRef, seed }: { exit: number; seed: number; plumeRef: (g: THREE.Group | null) => void }) {
  const [outer, inner] = useMemo(() => {
    const a = plumeMaterial();
    const b = plumeMaterial();
    a.uniforms.uSeed.value = seed;
    b.uniforms.uSeed.value = seed + 0.5;
    return [a, b];
  }, [seed]);
  return (
    <group ref={plumeRef} visible={false} userData={{ materials: [outer, inner] }}>
      <mesh position-y={-0.5} material={outer}>
        <cylinderGeometry args={[exit, exit * 0.2, 1, 20, 10, true]} />
      </mesh>
      <mesh position-y={-0.3} scale={[0.55, 0.6, 0.55]} material={inner}>
        <cylinderGeometry args={[exit, exit * 0.1, 1, 16, 8, true]} />
      </mesh>
    </group>
  );
}

function updatePlume(g: THREE.Group | null, on: boolean, throttle: number, vacuum: number, time: number, length = 1) {
  if (!g) return;
  g.visible = on && !skin.on;
  if (!g.visible) return;
  g.scale.set(1, (18 + 34 * throttle) * length, 1);
  for (const m of g.userData.materials as THREE.ShaderMaterial[]) {
    m.uniforms.uTime.value = time;
    m.uniforms.uThrottle.value = throttle;
    m.uniforms.uVacuum.value = vacuum;
  }
}

/** The glow of entry heating on the windward side. */
function HeatShell({ radius, bottom, top, pose }: { radius: number; bottom: number; top: number; pose: PoseRef }) {
  const material = useMemo(() => heatMaterial(), []);
  const mesh = useRef<THREE.Mesh>(null);
  useFrame(({ clock }) => {
    const m = mesh.current;
    if (!m) return;
    const p = pose.current;
    const heat = p?.heat ?? 0;
    m.visible = heat > 0.01 && !skin.on;
    const u = (m.material as THREE.ShaderMaterial).uniforms;
    u.uHeat.value = heat;
    u.uTime.value = clock.elapsedTime;
    if (p) local(p.flowBody, u.uFlow.value as THREE.Vector3);
  });
  return (
    <mesh ref={mesh} position-y={(bottom + top) / 2} material={material} visible={false}>
      <capsuleGeometry args={[radius, top - bottom - 2 * radius, 12, 40]} />
    </mesh>
  );
}

function useSteel(rings: number) {
  return useMemo(() => {
    const t = steelTextures(rings);
    return new THREE.MeshStandardMaterial({
      map: t.map,
      roughnessMap: t.roughnessMap,
      color: "#e4e6ea",
      metalness: 1,
      roughness: 0.62,
      envMapIntensity: 1.25,
      side: THREE.DoubleSide,
    });
  }, [rings]);
}

/** Cold-gas puffs: `spots` are [height, direction] in model axes. */
function Rcs({ spots, radius, pose }: { spots: { y: number; dir: Vec3 }[]; radius: number; pose: PoseRef }) {
  const puffs = useRef<(THREE.Mesh | null)[]>([]);
  useFrame(() => {
    const firing = pose.current?.rcs ?? [];
    puffs.current.forEach((m, i) => m && (m.visible = !!firing[i] && !skin.on));
  });
  return (
    <>
      {spots.map((r, i) => (
        <mesh
          key={i}
          ref={(m) => void (puffs.current[i] = m)}
          visible={false}
          position={[r.dir[0] * (radius + 2.2), r.y, r.dir[2] * (radius + 2.2)]}
          quaternion={new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, -1, 0), new THREE.Vector3(...r.dir))}
        >
          <coneGeometry args={[1.6, 4.5, 12, 1, true]} />
          <meshBasicMaterial color="#ffffff" transparent opacity={0.4} depthWrite={false} side={THREE.DoubleSide} />
        </mesh>
      ))}
    </>
  );
}

// ───────────────────────────── Ship ─────────────────────────────

const SHIP = { length: 52, radius: 4.5, cm: 24, bodyTop: 14, pin: 40 };
const FLAP_STOW = THREE.MathUtils.degToRad(75);
/** Where the flaps' hinges are centred, model height, m: the forward pair on the nose, the aft pair at the base. */
const FORE_FLAP_Y = 20.5;
const AFT_FLAP_Y = -17.2;

/**
 * The nose: a tangent ogive (the curve leaves the cylinder without a
 * corner and comes to a point), its tip rounded off, as on the real ship.
 */
function noseProfile(r: number, from: number, to: number): THREE.Vector2[] {
  const pts: THREE.Vector2[] = [];
  const length = to - from;
  const rho = (r * r + length * length) / (2 * r);
  const n = 36;
  const tip = 0.55;
  for (let i = 0; i <= n; i++) {
    const x = (i / n) * length;
    const radius = Math.sqrt(Math.max(0, rho * rho - x * x)) + r - rho;
    if (radius < tip) {
      // Round the point: a short cap instead of a needle.
      pts.push(new THREE.Vector2(tip, from + x));
      pts.push(new THREE.Vector2(tip * 0.7, from + x + 0.28));
      pts.push(new THREE.Vector2(0, from + x + 0.42));
      return pts;
    }
    pts.push(new THREE.Vector2(radius, from + x));
  }
  pts.push(new THREE.Vector2(0, to));
  return pts;
}

/** Radius of the ship's hull at model height `y` (cylinder below `bodyTop`, ogive above). */
function shipRadius(y: number): number {
  if (y <= SHIP.bodyTop) return SHIP.radius;
  const length = SHIP.length - SHIP.cm - SHIP.bodyTop;
  const rho = (SHIP.radius * SHIP.radius + length * length) / (2 * SHIP.radius);
  const x = Math.min(length, y - SHIP.bodyTop);
  return Math.max(0, Math.sqrt(Math.max(0, rho * rho - x * x)) + SHIP.radius - rho);
}

/**
 * A flap: a flat plate hinged along the hull, its leading (nose-side)
 * edge swept back, with a steel fairing over the hinge. Black like the
 * heat shield it extends. `lead` and `trail` are how far the tip's ends
 * are cut back from the root's, m.
 */
function Flap({ length, span, lead, trail, side, flapRef }: { length: number; span: number; lead: number; trail: number; side: 1 | -1; flapRef: (g: THREE.Group | null) => void }) {
  const shape = useMemo(() => {
    const s = new THREE.Shape();
    s.moveTo(0, -length / 2);
    s.lineTo(span, -length / 2 + trail);
    s.lineTo(span, length / 2 - lead);
    // A rounded forward corner.
    s.quadraticCurveTo(span * 0.55, length / 2 - lead * 0.35, 0, length / 2);
    s.closePath();
    return s;
  }, [length, span, lead, trail]);
  return (
    <group ref={flapRef}>
      <mesh rotation-y={side > 0 ? Math.PI / 2 : -Math.PI / 2} position-x={-0.2} castShadow>
        <extrudeGeometry args={[shape, { depth: 0.4, bevelEnabled: true, bevelSize: 0.1, bevelThickness: 0.1, bevelSegments: 2 }]} />
        <meshStandardMaterial color="#121214" metalness={0.2} roughness={0.8} />
      </mesh>
      {/* Hinge fairing along the root. */}
      <mesh castShadow>
        <capsuleGeometry args={[0.62, length - 1.6, 6, 14]} />
        <meshStandardMaterial color="#b9bdc3" metalness={0.95} roughness={0.5} />
      </mesh>
    </group>
  );
}

export function ShipModel({ pose }: { pose: PoseRef }) {
  const steel = useSteel(21);
  const tiles = useMemo(() => tileTexture([10, 22]), []);
  const noseTiles = useMemo(() => tileTexture([10, 8]), []);
  const flaps = useRef<(THREE.Group | null)[]>([]);
  const gimbal = useRef<THREE.Group>(null);
  const plumes = useRef<(THREE.Group | null)[]>([]);
  const shown = useRef([0.5, 0.5, 0.5, 0.5]);
  const base = -SHIP.cm;
  const noseTop = SHIP.length - SHIP.cm;
  const R = SHIP.radius;
  const bodyHeight = SHIP.bodyTop - base;

  useFrame(({ clock }, dt) => {
    const p = pose.current;
    const want = p?.surfaces ?? [0.5, 0.5, 0.5, 0.5];
    flaps.current.forEach((f, i) => {
      if (!f) return;
      shown.current[i] += (want[i] - shown.current[i]) * Math.min(1, dt * 8);
      f.rotation.y = (i % 2 === 0 ? 1 : -1) * (1 - shown.current[i]) * FLAP_STOW;
    });
    if (gimbal.current && p) {
      local(p.thrust, tmp);
      if (tmp.lengthSq() > 1) gimbal.current.quaternion.setFromUnitVectors(Y, tmp.normalize());
    }
    plumes.current.forEach((g, i) => updatePlume(g, p?.engines[i] === "running", p?.throttle ?? 0, p?.vacuum ?? 0, clock.elapsedTime));
  });

  const sl = [0, 120, 240].map((d) => THREE.MathUtils.degToRad(d));
  const vac = [60, 180, 300].map((d) => THREE.MathUtils.degToRad(d));
  return (
    <group>
      <Part id="tanks">
        <mesh position-y={base + bodyHeight / 2} castShadow receiveShadow material={steel}>
          <cylinderGeometry args={[R, R, bodyHeight, 72, 1, true]} />
        </mesh>
      </Part>
      <Part id="avionics">
        <mesh castShadow material={steel}>
          <latheGeometry args={[noseProfile(R, SHIP.bodyTop, noseTop), 72]} />
        </mesh>
      </Part>
      <Part id="shield">
        {/* Heat shield on the belly half (+X). */}
        <mesh position-y={base + bodyHeight / 2 + 0.8}>
          <cylinderGeometry args={[R + 0.05, R + 0.05, bodyHeight - 1.6, 72, 1, true, 0, Math.PI]} />
          <meshStandardMaterial map={tiles} roughness={0.82} metalness={0.05} side={THREE.DoubleSide} />
        </mesh>
        <mesh>
          <latheGeometry args={[noseProfile(R + 0.05, SHIP.bodyTop, noseTop + 0.05).slice(0, 30), 72, 0, Math.PI]} />
          <meshStandardMaterial map={noseTiles} roughness={0.82} metalness={0.05} side={THREE.DoubleSide} />
        </mesh>
      </Part>
      {/* Leeward side (−X): the raceway down the tanks and the payload slot in the nose section. */}
      <group rotation-y={THREE.MathUtils.degToRad(196)}>
        <mesh position={[R + 0.12, base + bodyHeight / 2 - 2, 0]}>
          <boxGeometry args={[0.26, bodyHeight - 9, 0.8]} />
          <meshStandardMaterial color="#a9adb3" metalness={0.95} roughness={0.5} />
        </mesh>
      </group>
      <mesh position-y={SHIP.bodyTop - 3.2}>
        <cylinderGeometry args={[R + 0.04, R + 0.04, 0.85, 40, 1, true, Math.PI * 1.5 - 0.42, 0.84]} />
        <meshStandardMaterial color="#2a2c30" metalness={0.7} roughness={0.6} side={THREE.DoubleSide} />
      </mesh>
      {/* Aft skirt: a darker band where the engine bay is. */}
      <mesh position-y={base + 1.3}>
        <cylinderGeometry args={[R + 0.03, R + 0.03, 2.6, 72, 1, true, Math.PI, Math.PI]} />
        <meshStandardMaterial color="#8d9197" metalness={0.95} roughness={0.55} side={THREE.DoubleSide} />
      </mesh>
      {[1, -1].map((side, k) => (
        <group key={`f${k}`} position={[-1.3, FORE_FLAP_Y, -side * (shipRadius(FORE_FLAP_Y) - 0.15)]}>
          <Part id={`flap${k}`}>
            <Flap length={7.4} span={4.4} lead={3.6} trail={0.5} side={side as 1 | -1} flapRef={(g) => void (flaps.current[k] = g)} />
          </Part>
        </group>
      ))}
      {[1, -1].map((side, k) => (
        <group key={`a${k}`} position={[-0.6, AFT_FLAP_Y, -side * (R - 0.1)]}>
          <Part id={`flap${2 + k}`}>
            <Flap length={13} span={5.4} lead={4.2} trail={0.3} side={side as 1 | -1} flapRef={(g) => void (flaps.current[2 + k] = g)} />
          </Part>
        </group>
      ))}
      <Part id="pins">
        {/* Catch pins (body ±y). */}
        {[1, -1].map((s) => (
          <mesh key={s} position={[0, SHIP.pin - SHIP.cm, s * (R + 0.6)]} rotation-x={Math.PI / 2}>
            <cylinderGeometry args={[0.45, 0.45, 1.3, 16]} />
            <meshStandardMaterial color="#6b6b70" metalness={0.8} roughness={0.4} />
          </mesh>
        ))}
      </Part>
      <Part id="engines">
        <mesh position-y={base + 0.2} rotation-x={Math.PI / 2}>
          <circleGeometry args={[R - 0.1, 48]} />
          <meshStandardMaterial color="#26262a" metalness={0.6} roughness={0.7} side={THREE.DoubleSide} />
        </mesh>
        {vac.map((a, i) => (
          <group key={`v${i}`} position={[3.15 * Math.sin(a), base + 0.2, 3.15 * Math.cos(a)]}>
            <Bell throat={0.5} exit={1.2} height={3.2} />
          </group>
        ))}
        <group position-y={base + 0.2}>
          <group ref={gimbal}>
            {sl.map((a, i) => (
              <group key={`s${i}`} position={[1.55 * Math.sin(a), 0, 1.55 * Math.cos(a)]}>
                <Bell throat={0.35} exit={0.65} height={1.9} />
                <group position-y={-1.9}>
                  <Plume exit={0.65} seed={i} plumeRef={(g) => void (plumes.current[i] = g)} />
                </group>
              </group>
            ))}
          </group>
        </group>
      </Part>
      <Part id="rcs">
        <Rcs
          radius={R}
          pose={pose}
          spots={[
            { y: 19, dir: [-1, 0, 0] },
            { y: 19, dir: [1, 0, 0] },
            { y: 19, dir: [0, 0, 1] },
            { y: 19, dir: [0, 0, -1] },
            { y: -17, dir: [0, 0, 1] },
            { y: -17, dir: [0, 0, -1] },
          ]}
        />
      </Part>
      <HeatShell radius={R + 0.9} bottom={base - 1} top={noseTop + 1} pose={pose} />
    </group>
  );
}

// ──────────────────────────── Booster ────────────────────────────

export const BOOSTER = { length: 72.3, radius: 4.5, cm: 26, pin: 62, finStation: 34 };

/** Patchy frost on the cold lower (oxygen) tank. */
function frostTexture(): THREE.Texture {
  const c = document.createElement("canvas");
  c.width = 256;
  c.height = 256;
  const g = c.getContext("2d")!;
  for (let x = 0; x < 256; x += 2) {
    const top = 30 + 90 * hash(x * 0.11) + 40 * hash(x * 0.9);
    const grad = g.createLinearGradient(0, top, 0, 256);
    grad.addColorStop(0, "rgba(240,246,250,0)");
    grad.addColorStop(0.25, `rgba(240,246,250,${0.35 + 0.3 * hash(x * 0.5)})`);
    grad.addColorStop(1, "rgba(240,246,250,0.75)");
    g.fillStyle = grad;
    g.fillRect(x, top, 2, 256 - top);
  }
  const t = new THREE.CanvasTexture(c);
  t.wrapS = THREE.RepeatWrapping;
  t.repeat.set(3, 1);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

/** A grid fin: a lattice plate on a shaft, turning about its radial axis. */
function GridFin({ azimuth, finRef, grid }: { azimuth: number; grid: THREE.Texture; finRef: (g: THREE.Group | null) => void }) {
  const R = BOOSTER.radius;
  return (
    <group rotation-y={azimuth} position-y={BOOSTER.finStation + 4}>
      {/* Radial direction is local +X here; the fin turns about it. */}
      <group position-x={R} ref={finRef}>
        <mesh position-x={0.5} rotation-z={Math.PI / 2}>
          <cylinderGeometry args={[0.35, 0.45, 1.2, 12]} />
          <meshStandardMaterial color="#6f7378" metalness={0.9} roughness={0.4} />
        </mesh>
        <mesh position-x={3.4} rotation-x={Math.PI / 2} castShadow>
          <planeGeometry args={[4.8, 6.2]} />
          <meshStandardMaterial map={grid} alphaTest={0.5} color="#9a9da2" metalness={0.85} roughness={0.45} side={THREE.DoubleSide} />
        </mesh>
        {[-0.35, 0.35].map((y) => (
          <mesh key={y} position={[3.4, y, 0]} rotation-x={Math.PI / 2}>
            <planeGeometry args={[4.8, 6.2]} />
            <meshStandardMaterial map={grid} alphaTest={0.5} color="#7d8085" metalness={0.85} roughness={0.5} side={THREE.DoubleSide} />
          </mesh>
        ))}
      </group>
    </group>
  );
}

export function BoosterModel({ pose }: { pose: PoseRef }) {
  const steel = useSteel(40);
  const grid = useMemo(() => gridTexture(), []);
  const frost = useMemo(() => frostTexture(), []);
  const engines = useMemo(() => engineLayout("booster"), []);
  const fins = useRef<(THREE.Group | null)[]>([]);
  const gimbals = useRef<(THREE.Group | null)[]>([]);
  const plumes = useRef<(THREE.Group | null)[]>([]);
  const jet = useRef<THREE.Group>(null);
  const glow = useRef<THREE.MeshBasicMaterial>(null);
  const q = useMemo(() => new THREE.Quaternion(), []);
  const R = BOOSTER.radius;
  const base = -BOOSTER.cm;
  const top = BOOSTER.length - BOOSTER.cm;

  useFrame(({ clock }) => {
    const p = pose.current;
    // Fin order matches the physics: body +y, −y, +x.
    fins.current.forEach((f, i) => f && (f.rotation.x = p?.surfaces[i] ?? 0));
    q.identity();
    if (p) {
      local(p.thrust, tmp);
      if (tmp.lengthSq() > 1) q.setFromUnitVectors(Y, tmp.normalize());
    }
    let lit = 0;
    for (let i = 0; i < 13; i++) {
      gimbals.current[i]?.quaternion.copy(q);
      const on = p?.engines[i] === "running";
      if (on) lit += 1;
      updatePlume(plumes.current[i], on, p?.throttle ?? 0, p?.vacuum ?? 0, clock.elapsedTime);
    }
    // Many engines side by side merge into one wide jet.
    updatePlume(jet.current, lit >= 2, p?.throttle ?? 0, p?.vacuum ?? 0, clock.elapsedTime, 1.5 + lit * 0.06);
    if (jet.current) {
      const wide = lit > 3 ? 1 : 0.42;
      jet.current.scale.x = wide;
      jet.current.scale.z = wide;
      jet.current.quaternion.copy(q);
    }
    if (glow.current) glow.current.opacity = Math.min(0.9, lit * 0.07 * (0.5 + (p?.throttle ?? 0)));
  });

  return (
    <group>
      <Part id="tanks">
        <mesh position-y={(base + top) / 2} castShadow receiveShadow material={steel}>
          <cylinderGeometry args={[R, R, BOOSTER.length, 72, 1, true]} />
        </mesh>
        {/* Frost on the oxygen tank. */}
        <mesh position-y={base + 21}>
          <cylinderGeometry args={[R + 0.03, R + 0.03, 40, 72, 1, true]} />
          <meshStandardMaterial map={frost} transparent opacity={0.5} roughness={1} metalness={0} depthWrite={false} />
        </mesh>
      </Part>
      <Part id="avionics">
        {/* Hot-stage ring: an open truss band at the top, and the forward dome. */}
        <mesh position-y={top + 0.9}>
          <cylinderGeometry args={[R, R, 1.8, 72, 1, true]} />
          <meshStandardMaterial map={grid} alphaTest={0.4} color="#55585d" metalness={0.8} roughness={0.5} side={THREE.DoubleSide} />
        </mesh>
        <mesh position-y={top} scale={[1, 0.22, 1]}>
          <sphereGeometry args={[R - 0.05, 48, 16, 0, Math.PI * 2, 0, Math.PI / 2]} />
          <meshStandardMaterial color="#8d9196" metalness={0.9} roughness={0.5} />
        </mesh>
      </Part>
      <Part id="shield">
        {/* Chines: four long strakes over the engine section. */}
        {[45, 135, 225, 315].map((d) => (
          <group key={d} rotation-y={THREE.MathUtils.degToRad(d)}>
            <mesh position={[R + 0.35, base + 10, 0]} castShadow material={steel}>
              <boxGeometry args={[0.9, 19, 1.6]} />
            </mesh>
          </group>
        ))}
        {/* Raceway. */}
        <group rotation-y={THREE.MathUtils.degToRad(200)}>
          <mesh position={[R + 0.15, 6, 0]}>
            <boxGeometry args={[0.3, 58, 0.9]} />
            <meshStandardMaterial color="#9da1a6" metalness={0.9} roughness={0.5} />
          </mesh>
        </group>
      </Part>
      {/* Grid fins at body +y (local −Z), −y (local +Z), +x. */}
      {[Math.PI / 2, -Math.PI / 2, 0].map((a, i) => (
        <Part key={i} id={`fin${i}`}>
          <GridFin azimuth={a} grid={grid} finRef={(g) => void (fins.current[i] = g)} />
        </Part>
      ))}
      <Part id="pins">
        {/* Catch hardpoints under the ±y fins. */}
        {[1, -1].map((s) => (
          <mesh key={s} position={[0, BOOSTER.pin - BOOSTER.cm, s * (R + 0.7)]} rotation-x={Math.PI / 2}>
            <cylinderGeometry args={[0.5, 0.5, 1.5, 16]} />
            <meshStandardMaterial color="#5d6066" metalness={0.85} roughness={0.4} />
          </mesh>
        ))}
      </Part>
      <Part id="engines">
        {/* Engine section: dark shielding around the outer ring. */}
        <mesh position-y={base + 1.4}>
          <cylinderGeometry args={[R + 0.06, R + 0.06, 2.8, 72, 1, true]} />
          <meshStandardMaterial color="#3b3d42" metalness={0.8} roughness={0.6} side={THREE.DoubleSide} />
        </mesh>
        <mesh position-y={base + 0.15} rotation-x={Math.PI / 2}>
          <circleGeometry args={[R - 0.05, 64]} />
          <meshStandardMaterial color="#1d1d20" metalness={0.5} roughness={0.8} side={THREE.DoubleSide} />
        </mesh>
        <mesh position-y={base - 0.4} rotation-x={Math.PI / 2}>
          <circleGeometry args={[R - 0.4, 48]} />
          <meshBasicMaterial ref={glow} color="#ffb070" transparent opacity={0} depthWrite={false} side={THREE.DoubleSide} blending={THREE.AdditiveBlending} />
        </mesh>
        <group position-y={base - 1.6}>
          <Plume exit={2.9} seed={7.7} plumeRef={(g) => void (jet.current = g)} />
        </group>
        {engines.map((e, i) => (
          <group key={i} position={[e.x, base + 0.1, e.z]}>
            <group ref={e.landing ? (g) => void (gimbals.current[i] = g) : undefined}>
              <Bell throat={0.3} exit={0.62} height={1.7} />
              {e.landing && (
                <group position-y={-1.7}>
                  <Plume exit={0.62} seed={i} plumeRef={(g) => void (plumes.current[i] = g)} />
                </group>
              )}
            </group>
          </group>
        ))}
      </Part>
      <Part id="rcs">
        <Rcs
          radius={R}
          pose={pose}
          spots={[
            { y: 40, dir: [-1, 0, 0] },
            { y: 40, dir: [1, 0, 0] },
            { y: 40, dir: [0, 0, 1] },
            { y: 40, dir: [0, 0, -1] },
            { y: -20, dir: [0, 0, 1] },
            { y: -20, dir: [0, 0, -1] },
          ]}
        />
      </Part>
      <HeatShell radius={R + 1.0} bottom={base - 2} top={top + 2} pose={pose} />
    </group>
  );
}
