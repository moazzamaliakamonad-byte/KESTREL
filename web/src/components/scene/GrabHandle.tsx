"use client";

import { Billboard } from "@react-three/drei";
import { useFrame, type ThreeEvent } from "@react-three/fiber";
import { useRef, useState } from "react";
import * as THREE from "three";
import { currentFlight } from "@/lib/flightRuntime";
import { live } from "@/lib/live";
import { toScene } from "@/lib/playback";
import { useApp } from "@/lib/store";

/** How far the visitor may carry the booster, m. */
const REACH = { x: 4000, zMin: 5, zMax: 3000 };

/**
 * Mid-flight "hand of god": drag the ring on the booster to move it. Time
 * stops while it is held (the simulator freezes); on release guidance
 * re-plans from wherever it was dropped. Dragging works in the vertical
 * x–z plane, like the start handles.
 */
export function GrabHandle() {
  const phase = useApp((s) => s.phase);
  const [held, setHeld] = useState(false);
  const marker = useRef<THREE.Group>(null);

  useFrame(({ camera }) => {
    const g = marker.current;
    const sample = live.sample;
    if (!g || !sample) return;
    g.position.set(...toScene(sample.position));
    g.scale.setScalar(camera.position.distanceTo(g.position) * 0.012);
  });

  if (phase !== "flying") return null;

  const begin = (e: ThreeEvent<PointerEvent>) => {
    e.stopPropagation();
    (e.target as Element).setPointerCapture?.(e.pointerId);
    setHeld(true);
    useApp.getState().setDragging(true);
  };
  const end = () => {
    if (!held) return;
    setHeld(false);
    useApp.getState().setDragging(false);
    currentFlight()?.release();
  };
  const move = (e: ThreeEvent<PointerEvent>) => {
    if (!held) return;
    e.stopPropagation();
    const y = live.sample?.position[1] ?? 0;
    const x = THREE.MathUtils.clamp(e.point.x, -REACH.x, REACH.x);
    const z = THREE.MathUtils.clamp(e.point.y, REACH.zMin, REACH.zMax);
    currentFlight()?.grab(x, y, z);
  };

  return (
    <group>
      <mesh
        visible={false}
        position={[0, 1500, 0]}
        onPointerMove={move}
        onPointerUp={end}
        onPointerLeave={end}
        raycast={held ? THREE.Mesh.prototype.raycast : () => null}
      >
        <planeGeometry args={[20000, 8000]} />
        <meshBasicMaterial side={THREE.DoubleSide} />
      </mesh>
      <group ref={marker}>
        <Billboard>
          <mesh onPointerDown={begin}>
            <circleGeometry args={[2.2, 32]} />
            <meshBasicMaterial transparent opacity={0} depthTest={false} />
          </mesh>
          <mesh>
            <ringGeometry args={[1.6, 1.8, 48]} />
            <meshBasicMaterial
              color={held ? "#ffffff" : "#7fe3ff"}
              transparent
              opacity={held ? 0.95 : 0.55}
              depthTest={false}
              toneMapped={false}
            />
          </mesh>
        </Billboard>
      </group>
    </group>
  );
}
