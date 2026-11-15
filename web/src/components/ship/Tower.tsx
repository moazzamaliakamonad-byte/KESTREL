"use client";

import { useFrame } from "@react-three/fiber";
import { useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { shipLive } from "@/lib/ship/live";
import { useShip } from "@/lib/ship/store";
import type { ScenarioInfo } from "@/lib/ship/types";

/**
 * The catch tower (Earth): a square lattice mast on the −x side of the
 * catch axis, a launch mount under the axis, and two long arms that swing
 * closed around the ship. Generic, procedural design at real scale (about
 * 145 m tall); no third-party assets.
 */

type Catch = NonNullable<ScenarioInfo["catch"]>;

const STEEL = "#3b3e44";
const SECTION = 9;

/** Lattice members as boxes: [position, quaternion, length]. */
function lattice(c: Catch): { p: THREE.Vector3; q: THREE.Quaternion; len: number }[] {
  const h = c.mast_half;
  const out: { p: THREE.Vector3; q: THREE.Quaternion; len: number }[] = [];
  const corners = [
    [-h, -h],
    [h, -h],
    [h, h],
    [-h, h],
  ];
  const member = (a: THREE.Vector3, b: THREE.Vector3) => {
    const d = b.clone().sub(a);
    const len = d.length();
    const q = new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 1, 0), d.normalize());
    out.push({ p: a.clone().add(b).multiplyScalar(0.5), q, len });
  };
  for (const [x, z] of corners) member(new THREE.Vector3(x, 0, z), new THREE.Vector3(x, c.tower_top, z));
  for (let y = 0; y < c.tower_top; y += SECTION) {
    const y2 = Math.min(y + SECTION, c.tower_top);
    for (let k = 0; k < 4; k++) {
      const [x1, z1] = corners[k];
      const [x2, z2] = corners[(k + 1) % 4];
      member(new THREE.Vector3(x1, y2, z1), new THREE.Vector3(x2, y2, z2));
      const flip = (y / SECTION) % 2 === 0;
      member(
        new THREE.Vector3(flip ? x1 : x2, y, flip ? z1 : z2),
        new THREE.Vector3(flip ? x2 : x1, y2, flip ? z2 : z1),
      );
    }
  }
  return out;
}

export function Tower() {
  const info = useShip((s) => s.info.earth);
  const c = info?.catch;
  if (!info || !c) return null;
  return <TowerBody c={c} radius={info.ship.radius} />;
}

function TowerBody({ c, radius }: { c: Catch; radius: number }) {
  const members = useMemo(() => lattice(c), [c]);
  const mesh = useRef<THREE.InstancedMesh>(null);
  const arms = useRef<(THREE.Group | null)[]>([]);
  const shown = useRef(0);

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

  // Arms: pivots on the mast's +x face; open they splay 30° apart.
  const face = c.mast_x + c.mast_half;
  const reach = -face + 12;
  useFrame((_, dt) => {
    const closed = useShip.getState().mode !== "editing" && !!shipLive.snapshot?.arms;
    shown.current += ((closed ? 1 : 0) - shown.current) * Math.min(1, dt * 1.2);
    const gap = radius + 0.6;
    arms.current.forEach((a, i) => {
      if (!a) return;
      const side = i === 0 ? 1 : -1;
      const closedAngle = Math.atan2(gap, reach);
      const openAngle = THREE.MathUtils.degToRad(32);
      a.rotation.y = side * (closedAngle + (openAngle - closedAngle) * (1 - shown.current));
    });
  });

  return (
    <group>
      <group position={[c.mast_x, 0, 0]}>
        <instancedMesh ref={mesh} args={[undefined, undefined, members.length]} castShadow receiveShadow>
          <boxGeometry args={[0.7, 1, 0.7]} />
          <meshStandardMaterial color={STEEL} metalness={0.6} roughness={0.6} />
        </instancedMesh>
        {/* Lightning mast on top. */}
        <mesh position-y={c.tower_top + 6}>
          <cylinderGeometry args={[0.2, 0.4, 12, 8]} />
          <meshStandardMaterial color={STEEL} />
        </mesh>
        {/* Carriage riding the mast at arm height. */}
        <mesh position={[0, c.arm_z, 0]} castShadow>
          <boxGeometry args={[c.mast_half * 2 + 2, 6, c.mast_half * 2 + 2]} />
          <meshStandardMaterial color="#2c2f34" metalness={0.5} roughness={0.6} />
        </mesh>
      </group>
      {/* The two arms ("chopsticks"). */}
      {[0, 1].map((i) => (
        <group key={i} position={[face, c.arm_z, (i === 0 ? -1 : 1) * 3.5]} ref={(g) => (arms.current[i] = g)}>
          <mesh position={[reach / 2, 0, 0]} castShadow>
            <boxGeometry args={[reach, 3.2, 1.6]} />
            <meshStandardMaterial color="#4a4e55" metalness={0.55} roughness={0.55} />
          </mesh>
          <mesh position={[reach / 2, 1.8, 0]}>
            <boxGeometry args={[reach * 0.9, 0.4, 2.2]} />
            <meshStandardMaterial color="#d9a441" metalness={0.2} roughness={0.7} />
          </mesh>
        </group>
      ))}
      {/* Launch mount under the catch axis. */}
      <group>
        {[0, 60, 120, 180, 240, 300].map((d) => {
          const a = THREE.MathUtils.degToRad(d);
          return (
            <mesh key={d} position={[9 * Math.cos(a), 10, 9 * Math.sin(a)]} castShadow>
              <boxGeometry args={[2.2, 20, 2.2]} />
              <meshStandardMaterial color="#8d8a85" roughness={0.9} />
            </mesh>
          );
        })}
        <mesh position-y={21} castShadow receiveShadow>
          <cylinderGeometry args={[11, 11, 3, 6, 1, false]} />
          <meshStandardMaterial color="#9a958e" roughness={0.85} />
        </mesh>
      </group>
    </group>
  );
}
