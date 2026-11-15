"use client";

import { Billboard } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import { quatToScene, toScene, type PQuat, type Vec3 } from "@/lib/ship/frames";
import { shipLive } from "@/lib/ship/live";
import { useShip } from "@/lib/ship/store";
import type { ShipSnapshot } from "@/lib/ship/types";

/**
 * A generic Starship-class ship, drawn procedurally at real scale (52 m ×
 * 9 m): stainless body, black hexagonal tiles on the belly (+X, the
 * windward side in the belly flop), two forward and two aft flaps, three
 * gimballed sea-level bells and three larger fixed vacuum bells, catch pins
 * near the forward flaps, and six landing legs for Mars. Original design,
 * no third-party assets (D-010). The group origin is the centre of mass.
 */

const L = 52;
const R = 4.5;
const CM = 24;
const BASE = -CM;
const BODY_TOP = 14;
const NOSE_TOP = L - CM;
const PIN_Y = 40 - CM;
const SL_RADIUS = 1.6;
const VAC_RADIUS = 3.15;
const FLAP_STOW = THREE.MathUtils.degToRad(75);
/** Belly down, nose toward −x (Rust `ship_control::belly_attitude(0)`). */
const BELLY_DOWN: PQuat = [0, -Math.SQRT1_2, 0, Math.SQRT1_2];

function hexTexture(): THREE.CanvasTexture {
  const c = document.createElement("canvas");
  c.width = 256;
  c.height = 256;
  const g = c.getContext("2d")!;
  g.fillStyle = "#121214";
  g.fillRect(0, 0, 256, 256);
  const s = 16;
  const h = s * Math.sqrt(3);
  g.strokeStyle = "#2a2a2e";
  g.lineWidth = 2;
  for (let row = -1; row < 256 / h + 1; row++) {
    for (let col = -1; col < 256 / (1.5 * s) + 1; col++) {
      const cx = col * 1.5 * s;
      const cy = row * h + (col % 2 ? h / 2 : 0);
      g.beginPath();
      for (let k = 0; k < 6; k++) {
        const a = (Math.PI / 3) * k;
        g.lineTo(cx + s * Math.cos(a), cy + s * Math.sin(a));
      }
      g.closePath();
      g.stroke();
    }
  }
  const t = new THREE.CanvasTexture(c);
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

function noseProfile(r: number): THREE.Vector2[] {
  const pts: THREE.Vector2[] = [];
  const n = 24;
  for (let i = 0; i <= n; i++) {
    const f = i / n;
    // Tangent ogive from the body radius to a small rounded tip.
    const rr = Math.max(0.15, r * Math.sqrt(1 - f * f * 0.98));
    pts.push(new THREE.Vector2(rr, BODY_TOP + f * (NOSE_TOP - BODY_TOP)));
  }
  pts.push(new THREE.Vector2(0, NOSE_TOP + 0.2));
  return pts;
}

/** A flap: plate in the local Y–Z plane, hinged along Y at its inner edge. */
function Flap({ length, span, sweep, flapRef, side }: { length: number; span: number; sweep: number; side: 1 | -1; flapRef: (g: THREE.Group | null) => void }) {
  const shape = useMemo(() => {
    const s = new THREE.Shape();
    s.moveTo(0, -length / 2);
    s.lineTo(span, -length / 2 + sweep);
    s.lineTo(span, length / 2 - 0.5);
    s.lineTo(0, length / 2);
    s.closePath();
    return s;
  }, [length, span, sweep]);
  return (
    <group ref={flapRef}>
      <mesh rotation-y={side > 0 ? Math.PI / 2 : -Math.PI / 2} castShadow>
        <extrudeGeometry args={[shape, { depth: 0.35, bevelEnabled: false }]} />
        <meshStandardMaterial color="#1a1a1d" metalness={0.3} roughness={0.7} />
      </mesh>
    </group>
  );
}

function Bell({ top, bottom, height }: { top: number; bottom: number; height: number }) {
  return (
    <mesh position-y={-height / 2} castShadow>
      <cylinderGeometry args={[top, bottom, height, 28, 1, true]} />
      <meshStandardMaterial color="#5b5550" metalness={0.8} roughness={0.35} side={THREE.DoubleSide} emissive="#ff7a2a" emissiveIntensity={0} />
    </mesh>
  );
}

const plumeMaterial = () =>
  new THREE.MeshBasicMaterial({
    color: "#9fc4ff",
    transparent: true,
    opacity: 0.85,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    side: THREE.DoubleSide,
  });

export function Ship() {
  const root = useRef<THREE.Group>(null);
  const halo = useRef<THREE.Group>(null);
  const flaps = useRef<(THREE.Group | null)[]>([]);
  const gimbal = useRef<THREE.Group>(null);
  const plumes = useRef<(THREE.Group | null)[]>([]);
  const rcs = useRef<(THREE.Mesh | null)[]>([]);
  const legs = useRef<(THREE.Group | null)[]>([]);
  const legsRoot = useRef<THREE.Group>(null);
  const tiles = useMemo(() => {
    const t = hexTexture();
    t.repeat.set(18, 30);
    return t;
  }, []);
  const plumeMat = useMemo(() => plumeMaterial(), []);
  const coreMat = useMemo(() => {
    const m = plumeMaterial();
    m.color.set("#ffffff");
    return m;
  }, []);
  const deploy = useRef(0);
  const flapShown = useRef([0.5, 0.5, 0.5, 0.5]);

  useFrame(({ camera }, dt) => {
    const g = root.current;
    if (!g) return;
    const st = useShip.getState();
    const s: ShipSnapshot | null = st.mode === "editing" ? null : shipLive.snapshot;
    const position: Vec3 = s?.position ?? st.start.position;
    const q: PQuat = s?.quat ?? BELLY_DOWN;
    g.position.set(...toScene(position));
    g.quaternion.set(...quatToScene(q));
    // Far away the ship is a few pixels: show a ring that keeps its size.
    if (halo.current) {
      const dist = camera.position.distanceTo(g.position);
      halo.current.visible = dist > 1500;
      halo.current.position.copy(g.position);
      halo.current.scale.setScalar(dist * 0.02);
    }

    // Flaps: exposure 0 → stowed back toward the leeward side, 1 → straight out.
    const want = s?.flaps ?? [0.5, 0.5, 0.5, 0.5];
    flaps.current.forEach((f, i) => {
      if (!f) return;
      flapShown.current[i] += (want[i] - flapShown.current[i]) * Math.min(1, dt * 8);
      const side = i % 2 === 0 ? 1 : -1;
      f.rotation.y = side * (1 - flapShown.current[i]) * FLAP_STOW;
    });

    // Gimbal: point the sea-level bells along the body-frame thrust.
    const t = s?.thrust ?? [0, 0, 1];
    if (gimbal.current) {
      const dir = new THREE.Vector3(t[0], t[2], -t[1]);
      if (dir.lengthSq() > 1) {
        dir.normalize();
        gimbal.current.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir);
      }
    }
    const perEngine = 2.45e6;
    let running = 0;
    s?.engines.forEach((e) => (running += e === "running" ? 1 : 0));
    const each = running > 0 ? (s?.thrust_mag ?? 0) / running / perEngine : 0;
    plumes.current.forEach((p, i) => {
      if (!p) return;
      const on = s?.engines[i] === "running";
      p.visible = on;
      if (on) {
        const flicker = 0.92 + 0.08 * Math.sin(performance.now() * 0.05 + i * 2);
        const len = (14 + 26 * each) * flicker;
        p.scale.set(1 + 0.4 * each, len, 1 + 0.4 * each);
      }
    });
    (s?.rcs ?? []).forEach((on, i) => {
      const m = rcs.current[i];
      if (m) m.visible = on;
    });
    const legsWanted = s?.legs ? 1 : 0;
    deploy.current += (legsWanted - deploy.current) * Math.min(1, dt * 1.5);
    if (legsRoot.current) legsRoot.current.visible = st.scenario === "mars";
    legs.current.forEach((l) => l && (l.rotation.z = THREE.MathUtils.degToRad(2 + 33 * deploy.current)));
  });

  const bodyHeight = BODY_TOP - BASE;
  const slAngles = [0, 120, 240].map((d) => THREE.MathUtils.degToRad(d));
  const vacAngles = [60, 180, 300].map((d) => THREE.MathUtils.degToRad(d));
  // RCS: pitch ± and yaw ± (forward group), roll ± (aft group); puff points
  // opposite the push.
  const rcsSpots: { y: number; dir: Vec3 }[] = [
    { y: 19, dir: [-1, 0, 0] },
    { y: 19, dir: [1, 0, 0] },
    { y: 19, dir: [0, 0, 1] },
    { y: 19, dir: [0, 0, -1] },
    { y: -17, dir: [0, 0, 1] },
    { y: -17, dir: [0, 0, -1] },
  ];

  return (
    <>
    <group ref={halo}>
      <Billboard>
        <mesh>
          <ringGeometry args={[0.8, 1, 48]} />
          <meshBasicMaterial color="#7fe3ff" transparent opacity={0.85} depthTest={false} />
        </mesh>
      </Billboard>
    </group>
    <group ref={root}>
      {/* Stainless body and nose. */}
      <mesh position-y={BASE + bodyHeight / 2} castShadow receiveShadow>
        <cylinderGeometry args={[R, R, bodyHeight, 64, 1, true]} />
        <meshStandardMaterial color="#d7dade" metalness={0.55} roughness={0.38} side={THREE.DoubleSide} />
      </mesh>
      <mesh castShadow>
        <latheGeometry args={[noseProfile(R), 64]} />
        <meshStandardMaterial color="#d7dade" metalness={0.55} roughness={0.38} />
      </mesh>
      {/* Heat shield on the belly half (+X): theta 0..π covers x ≥ 0. */}
      <mesh position-y={BASE + bodyHeight / 2 + 1}>
        <cylinderGeometry args={[R + 0.04, R + 0.04, bodyHeight - 2, 64, 1, true, 0, Math.PI]} />
        <meshStandardMaterial map={tiles} color="#ffffff" roughness={0.85} metalness={0.05} side={THREE.DoubleSide} />
      </mesh>
      <mesh>
        <latheGeometry args={[noseProfile(R + 0.04).slice(0, 18), 64, 0, Math.PI]} />
        <meshStandardMaterial color="#141416" roughness={0.85} side={THREE.DoubleSide} />
      </mesh>
      {/* Forward flaps (smaller, set toward the leeward side), aft flaps. */}
      {[1, -1].map((side, k) => (
        <group key={`f${k}`} position={[-2.2, 22, -side * (R - 1.2)]}>
          <Flap length={8} span={4.2} sweep={2.5} side={side as 1 | -1} flapRef={(g) => (flaps.current[k] = g)} />
        </group>
      ))}
      {[1, -1].map((side, k) => (
        <group key={`a${k}`} position={[0, -19, -side * R]}>
          <Flap length={10} span={5} sweep={1} side={side as 1 | -1} flapRef={(g) => (flaps.current[2 + k] = g)} />
        </group>
      ))}
      {/* Catch pins (body ±y = local ∓Z). */}
      {[1, -1].map((s) => (
        <mesh key={s} position={[0, PIN_Y, s * (R + 0.6)]} rotation-x={Math.PI / 2}>
          <cylinderGeometry args={[0.45, 0.45, 1.3, 16]} />
          <meshStandardMaterial color="#6b6b70" metalness={0.8} roughness={0.4} />
        </mesh>
      ))}
      {/* Engine bay skirt. */}
      <mesh position-y={BASE + 0.6}>
        <cylinderGeometry args={[R, R - 0.2, 1.2, 64, 1, true]} />
        <meshStandardMaterial color="#8f9398" metalness={0.8} roughness={0.4} side={THREE.DoubleSide} />
      </mesh>
      {/* Vacuum bells: fixed, larger. */}
      {vacAngles.map((a, i) => (
        <group key={`v${i}`} position={[VAC_RADIUS * Math.sin(a), BASE + 0.2, VAC_RADIUS * Math.cos(a)]}>
          <Bell top={0.55} bottom={1.15} height={3.1} />
        </group>
      ))}
      {/* Sea-level bells on a shared gimbal, with their plumes. */}
      <group position-y={BASE + 0.2}>
        <group ref={gimbal}>
          {slAngles.map((a, i) => (
            <group key={`s${i}`} position={[SL_RADIUS * Math.sin(a), 0, SL_RADIUS * Math.cos(a)]}>
              <Bell top={0.4} bottom={0.65} height={1.9} />
              {/* Plume: wide at the nozzle exit, narrowing downstream. */}
              <group ref={(g) => (plumes.current[i] = g)} position-y={-1.9} visible={false}>
                <mesh position-y={-0.5} rotation-x={Math.PI} material={plumeMat}>
                  <coneGeometry args={[0.8, 1, 20, 1, true]} />
                </mesh>
                <mesh position-y={-0.35} rotation-x={Math.PI} scale={[0.5, 0.7, 0.5]} material={coreMat}>
                  <coneGeometry args={[0.8, 1, 16, 1, true]} />
                </mesh>
              </group>
            </group>
          ))}
        </group>
      </group>
      {/* Cold-gas RCS puffs. */}
      {rcsSpots.map((r, i) => (
        <mesh
          key={i}
          ref={(m) => (rcs.current[i] = m)}
          visible={false}
          position={[r.dir[0] * (R + 1.5), r.y, r.dir[2] * (R + 1.5)]}
          quaternion={new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, -1, 0), new THREE.Vector3(...r.dir))}
        >
          <coneGeometry args={[1.2, 3, 12, 1, true]} />
          <meshBasicMaterial color="#ffffff" transparent opacity={0.55} depthWrite={false} />
        </mesh>
      ))}
      {/* Mars legs: six struts that swing out. */}
      <group ref={legsRoot} position-y={BASE + 3}>
        {[0, 60, 120, 180, 240, 300].map((d, i) => (
          <group key={d} rotation-y={THREE.MathUtils.degToRad(d)}>
            <group position-x={R - 0.3} ref={(g) => (legs.current[i] = g)}>
              <mesh position-y={-3.2} castShadow>
                <boxGeometry args={[0.45, 6.4, 0.7]} />
                <meshStandardMaterial color="#3a3a3e" metalness={0.6} roughness={0.5} />
              </mesh>
              <mesh position-y={-6.4}>
                <cylinderGeometry args={[0.9, 1.0, 0.3, 16]} />
                <meshStandardMaterial color="#4a4a4e" metalness={0.5} roughness={0.6} />
              </mesh>
            </group>
          </group>
        ))}
      </group>
    </group>
    </>
  );
}
