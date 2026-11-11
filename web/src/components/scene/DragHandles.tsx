"use client";

import { Billboard, Line } from "@react-three/drei";
import { useFrame, type ThreeEvent } from "@react-three/fiber";
import { useRef, useState } from "react";
import * as THREE from "three";
import { clampPosition, LIMITS, velocityFromHandle } from "@/lib/drag";
import { toScene } from "@/lib/playback";
import { useApp } from "@/lib/store";
import type { Vec3 } from "@/lib/types";

type Grab = "body" | "arrow" | null;

const ARROW_COLOR = new THREE.Color("#ffb35c").multiplyScalar(1.8);

/**
 * Editing handles: drag the booster to move the start position, and the
 * arrow tip to set the start velocity. Both live in the vertical x–z plane
 * (physics y = 0), which is the scene plane z = 0.
 */
export function DragHandles() {
  const phase = useApp((s) => s.phase);
  const start = useApp((s) => s.start);
  const glideSlope = useApp((s) => s.vehicle?.glide_slope ?? (4 * Math.PI) / 180);
  const [grab, setGrab] = useState<Grab>(null);
  const tipMarker = useRef<THREE.Group>(null);
  const bodyMarker = useRef<THREE.Group>(null);

  const pos = toScene(start.position);
  const tip: Vec3 = toScene([
    start.position[0] + start.velocity[0] * LIMITS.handleSeconds,
    0,
    start.position[2] + start.velocity[2] * LIMITS.handleSeconds,
  ]);

  // Keep handle markers a constant size on screen.
  useFrame(({ camera }) => {
    for (const [ref, at] of [
      [tipMarker, tip],
      [bodyMarker, pos],
    ] as const) {
      if (!ref.current) continue;
      const d = camera.position.distanceTo(new THREE.Vector3(...at));
      ref.current.scale.setScalar(d * 0.012);
    }
  });

  if (phase !== "editing") return null;

  const begin = (what: Grab) => (e: ThreeEvent<PointerEvent>) => {
    e.stopPropagation();
    (e.target as Element).setPointerCapture?.(e.pointerId);
    setGrab(what);
    useApp.getState().setDragging(true);
  };
  const end = () => {
    setGrab(null);
    useApp.getState().setDragging(false);
  };
  const move = (e: ThreeEvent<PointerEvent>) => {
    if (!grab) return;
    e.stopPropagation();
    // Scene point on the plane z = 0 → physics (x, z) = (point.x, point.y).
    const { x, y: z } = e.point;
    const { dispatch, start: s } = useApp.getState();
    if (grab === "body") {
      const p = clampPosition(x, z, glideSlope);
      dispatch({ type: "setStart", start: { ...s, position: [p.x, 0, p.z] } });
    } else {
      const v = velocityFromHandle({ x: s.position[0], z: s.position[2] }, { x, z });
      dispatch({ type: "setStart", start: { ...s, velocity: [v.vx, 0, v.vz] } });
    }
  };

  return (
    <group>
      {/* Invisible drag surface, only catching events while dragging. */}
      <mesh
        visible={false}
        position={[0, 1500, 0]}
        onPointerMove={move}
        onPointerUp={end}
        onPointerLeave={end}
        raycast={grab ? THREE.Mesh.prototype.raycast : () => null}
      >
        <planeGeometry args={[20000, 8000]} />
        <meshBasicMaterial side={THREE.DoubleSide} />
      </mesh>

      <Line points={[pos, tip]} color={ARROW_COLOR} lineWidth={2.5} toneMapped={false} />

      <group position={pos}>
        <Billboard ref={bodyMarker}>
          <mesh onPointerDown={begin("body")}>
            <circleGeometry args={[2.2, 32]} />
            <meshBasicMaterial transparent opacity={0} depthTest={false} />
          </mesh>
          <mesh>
            <ringGeometry args={[1.6, 1.8, 48]} />
            <meshBasicMaterial
              color={grab === "body" ? "#ffffff" : "#7fe3ff"}
              transparent
              opacity={0.9}
              depthTest={false}
              toneMapped={false}
            />
          </mesh>
        </Billboard>
      </group>

      <group position={tip}>
        <Billboard ref={tipMarker}>
          <mesh onPointerDown={begin("arrow")}>
            <circleGeometry args={[1.4, 32]} />
            <meshBasicMaterial color={ARROW_COLOR} depthTest={false} toneMapped={false} />
          </mesh>
        </Billboard>
      </group>
    </group>
  );
}
