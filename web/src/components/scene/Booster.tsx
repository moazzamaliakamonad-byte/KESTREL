"use client";

import { Billboard } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useRef } from "react";
import * as THREE from "three";
import { smoothQuaternion, targetQuaternion, type Quat } from "@/lib/attitude";
import { legDeploy } from "@/lib/legs";
import { live } from "@/lib/live";
import { toScene } from "@/lib/playback";
import { Dust } from "./Dust";
import { Plume } from "./Plume";

const RADIUS = 1.85;
const BODY_BOTTOM = 1.6;
const BODY_TOP = 10.4;
const LEG_LENGTH = 5;
const LEG_HINGE_Y = 3.6;
/** Folded legs point up along the body; deployed they splay 45° from vertical. */
const LEG_FOLDED = 0;
const LEG_DEPLOYED = THREE.MathUtils.degToRad(135);
/** Displayed attitude eases to upright below this altitude (m). */
const UPRIGHT_BELOW = 40;
const IDENTITY = new THREE.Quaternion();

function Leg({ angle, legRef }: { angle: number; legRef: (g: THREE.Group | null) => void }) {
  return (
    <group rotation-y={angle}>
      <group position={[RADIUS, LEG_HINGE_Y, 0]} ref={legRef}>
        <mesh position-y={LEG_LENGTH / 2} castShadow>
          <boxGeometry args={[0.28, LEG_LENGTH, 0.5]} />
          <meshStandardMaterial color="#2d2c2f" metalness={0.6} roughness={0.45} />
        </mesh>
        <mesh position-y={LEG_LENGTH} castShadow>
          <cylinderGeometry args={[0.55, 0.65, 0.18, 16]} />
          <meshStandardMaterial color="#3b3a3d" metalness={0.5} roughness={0.5} />
        </mesh>
      </group>
    </group>
  );
}

/** An original booster design, driven by the live flight sample. */
export function Booster() {
  const root = useRef<THREE.Group>(null);
  const halo = useRef<THREE.Group>(null);
  const bell = useRef<THREE.MeshStandardMaterial>(null);
  const legs = useRef<(THREE.Group | null)[]>([]);
  const attitude = useRef<Quat>([0, 0, 0, 1]);

  useFrame(({ camera }, dt) => {
    const s = live.sample;
    const g = root.current;
    if (!s || !g) return;
    const [x, y, z] = toScene(s.position);
    g.position.set(x, y, z);
    // Body follows the thrust direction (a 3-DoF plan has no attitude of its
    // own), eased upright over the last 40 m: cosmetic, see LIMITATIONS.md.
    const along = targetQuaternion(s.thrust, attitude.current);
    const upright = 1 - Math.min(Math.max(s.position[2] / UPRIGHT_BELOW, 0), 1);
    const target = new THREE.Quaternion(...along).slerp(IDENTITY, upright);
    attitude.current = smoothQuaternion(attitude.current, [target.x, target.y, target.z, target.w], dt);
    g.quaternion.set(...attitude.current);

    const deploy = legDeploy(s.position[2]);
    const angle = LEG_FOLDED + (LEG_DEPLOYED - LEG_FOLDED) * deploy;
    for (const leg of legs.current) leg?.rotation.set(0, 0, -angle);

    const heat = s.thrustMag / live.maxThrust;
    if (bell.current) bell.current.emissiveIntensity = 0.2 + 2.5 * heat;

    if (halo.current) {
      const dist = camera.position.distanceTo(g.position);
      halo.current.visible = dist > 350;
      halo.current.scale.setScalar(dist * 0.018);
    }
  });

  return (
    <group ref={root}>
      {/* Body */}
      <mesh position-y={(BODY_BOTTOM + BODY_TOP) / 2} castShadow receiveShadow>
        <cylinderGeometry args={[RADIUS, RADIUS, BODY_TOP - BODY_BOTTOM, 40]} />
        <meshStandardMaterial color="#e8e4dc" metalness={0.25} roughness={0.42} />
      </mesh>
      {[3.2, 6.2, 9.0].map((y) => (
        <mesh key={y} position-y={y}>
          <cylinderGeometry args={[RADIUS + 0.02, RADIUS + 0.02, 0.07, 40]} />
          <meshStandardMaterial color="#9b968e" roughness={0.6} />
        </mesh>
      ))}
      <mesh position-y={BODY_TOP - 0.5}>
        <cylinderGeometry args={[RADIUS + 0.03, RADIUS + 0.03, 0.8, 40]} />
        <meshStandardMaterial color="#232226" metalness={0.4} roughness={0.5} />
      </mesh>
      {/* Nose */}
      <mesh position-y={BODY_TOP + 0.8} castShadow>
        <cylinderGeometry args={[0.6, RADIUS, 1.6, 40]} />
        <meshStandardMaterial color="#e8e4dc" metalness={0.25} roughness={0.42} />
      </mesh>
      {/* Grid fins */}
      {[0, 1, 2, 3].map((i) => (
        <group key={i} rotation-y={(i * Math.PI) / 2 + Math.PI / 4}>
          <mesh position={[RADIUS + 0.55, BODY_TOP - 1.4, 0]} castShadow>
            <boxGeometry args={[1.1, 0.9, 0.12]} />
            <meshStandardMaterial color="#47464b" metalness={0.7} roughness={0.35} wireframe={false} />
          </mesh>
        </group>
      ))}
      {/* Engine bell */}
      <mesh position-y={0.9} castShadow>
        <cylinderGeometry args={[0.55, 1.0, 1.4, 32, 1, true]} />
        <meshStandardMaterial
          ref={bell}
          color="#2a2524"
          metalness={0.8}
          roughness={0.35}
          emissive="#ff5a1f"
          emissiveIntensity={0.2}
          side={THREE.DoubleSide}
        />
      </mesh>
      {/* Legs */}
      {[0, 1, 2, 3].map((i) => (
        <Leg key={i} angle={(i * Math.PI) / 2} legRef={(g) => (legs.current[i] = g)} />
      ))}
      <Plume />
      <Dust />
      {/* Distance marker so the booster can be found from kilometres away */}
      <Billboard ref={halo} position-y={6}>
        <mesh>
          <ringGeometry args={[0.9, 1, 48]} />
          <meshBasicMaterial color="#7fe3ff" transparent opacity={0.85} toneMapped={false} depthTest={false} />
        </mesh>
      </Billboard>
    </group>
  );
}
