"use client";

import { PerformanceMonitor } from "@react-three/drei";
import { Canvas } from "@react-three/fiber";
import { useState, type ReactNode } from "react";
import * as THREE from "three";
import { Effects, type Quality } from "./Effects";
import { HORIZON_COLOR, Sky, SUN_DIRECTION } from "./Sky";

/** The 3D canvas: lighting, fog, sky, effects and adaptive quality. */
export function Stage({ children }: { children: ReactNode }) {
  const [dpr, setDpr] = useState(2);
  const [quality, setQuality] = useState<Quality>("high");
  const sun = SUN_DIRECTION.clone().multiplyScalar(3000);

  return (
    <Canvas
      shadows
      flat
      dpr={[1, dpr]}
      gl={{ antialias: false, powerPreference: "high-performance" }}
      camera={{ fov: 45, near: 1, far: 20000, position: [-900, 700, 3400] }}
      onCreated={({ scene }) => {
        scene.fog = new THREE.FogExp2(HORIZON_COLOR.clone().multiplyScalar(0.8), 0.00016);
      }}
    >
      <PerformanceMonitor
        onDecline={() => {
          if (dpr > 1) setDpr(1);
          else setQuality("low");
        }}
      />
      <Sky />
      <hemisphereLight args={["#d8a982", "#3a2018", 1.15]} />
      <directionalLight
        position={sun.toArray()}
        intensity={3.2}
        color="#ffd7b0"
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-camera-left={-120}
        shadow-camera-right={120}
        shadow-camera-top={120}
        shadow-camera-bottom={-120}
        shadow-camera-near={10}
        shadow-camera-far={8000}
        shadow-bias={-0.0004}
      />
      {children}
      <Effects quality={quality} />
    </Canvas>
  );
}
