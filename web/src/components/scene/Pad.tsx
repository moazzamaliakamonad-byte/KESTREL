"use client";

import { useFrame } from "@react-three/fiber";
import { useRef } from "react";
import * as THREE from "three";

const LIGHTS = 16;
const RADIUS = 30;

/** Landing pad: concrete disc, target markings and perimeter lights. */
export function Pad({ blinking }: { blinking: boolean }) {
  const lights = useRef<THREE.MeshBasicMaterial>(null);

  useFrame(({ clock }) => {
    if (!lights.current) return;
    const on = !blinking || Math.sin(clock.elapsedTime * Math.PI * 4) > 0;
    lights.current.color.setRGB(on ? 6 : 0.4, on ? 2.6 : 0.2, on ? 0.6 : 0.1);
  });

  return (
    <group>
      <mesh rotation-x={-Math.PI / 2} position-y={0.05} receiveShadow>
        <circleGeometry args={[RADIUS, 64]} />
        <meshStandardMaterial color="#7d776f" roughness={0.85} />
      </mesh>
      <mesh rotation-x={-Math.PI / 2} position-y={0.08}>
        <ringGeometry args={[RADIUS - 2.2, RADIUS - 1.2, 96]} />
        <meshStandardMaterial color="#e8e2d6" roughness={0.6} />
      </mesh>
      <mesh rotation-x={-Math.PI / 2} position-y={0.08}>
        <ringGeometry args={[7, 8.5, 64]} />
        <meshStandardMaterial color="#ff8a3d" roughness={0.5} emissive="#ff6a1a" emissiveIntensity={0.4} />
      </mesh>
      {[0, 1].map((i) => (
        <mesh key={i} rotation-x={-Math.PI / 2} rotation-z={(i * Math.PI) / 2} position-y={0.09}>
          <planeGeometry args={[2, 14]} />
          <meshStandardMaterial color="#e8e2d6" roughness={0.6} />
        </mesh>
      ))}
      <instancedMesh
        args={[undefined, undefined, LIGHTS]}
        ref={(m) => {
          if (!m) return;
          const mat = new THREE.Matrix4();
          for (let i = 0; i < LIGHTS; i++) {
            const a = (i / LIGHTS) * Math.PI * 2;
            mat.makeTranslation(Math.cos(a) * (RADIUS + 1), 0.5, Math.sin(a) * (RADIUS + 1));
            m.setMatrixAt(i, mat);
          }
          m.instanceMatrix.needsUpdate = true;
        }}
      >
        <sphereGeometry args={[0.45, 12, 8]} />
        <meshBasicMaterial ref={lights} toneMapped={false} />
      </instancedMesh>
    </group>
  );
}
