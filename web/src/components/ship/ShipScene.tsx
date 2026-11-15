"use client";

import { PerformanceMonitor } from "@react-three/drei";
import { Canvas, useFrame } from "@react-three/fiber";
import { useState } from "react";
import * as THREE from "three";
import { Effects, type Quality } from "@/components/scene/Effects";
import { shipFrame } from "@/lib/ship/runtime";
import { useShip } from "@/lib/ship/store";
import { Ship } from "./Ship";
import { ShipCameras } from "./ShipCameras";
import { ShipPaths } from "./ShipPaths";
import { Tower } from "./Tower";
import { World } from "./World";

/** Steps the closed loop once per rendered frame while flying. */
function Loop() {
  useFrame((_, dt) => {
    if (useShip.getState().mode === "flying") shipFrame(dt);
  });
  return null;
}

/** The 3D view: planet-dependent light and fog, world, tower, ship, paths. */
export function ShipScene() {
  const scenario = useShip((s) => s.scenario);
  const [dpr, setDpr] = useState(2);
  const [quality, setQuality] = useState<Quality>("high");
  const earth = scenario === "earth";
  const sun = earth ? new THREE.Vector3(2500, 4000, 1500) : new THREE.Vector3(-3000, 800, -1200);

  return (
    <Canvas
      shadows
      flat
      dpr={[1, dpr]}
      gl={{ antialias: false, powerPreference: "high-performance", logarithmicDepthBuffer: true }}
      camera={{ fov: 45, near: 1, far: 40000, position: [2000, 2500, 6000] }}
      onCreated={({ scene }) => {
        scene.fog = new THREE.FogExp2(earth ? "#c9dcea" : "#a37a5c", earth ? 0.00005 : 0.00009);
      }}
      key={scenario}
    >
      <PerformanceMonitor onDecline={() => (dpr > 1 ? setDpr(1) : setQuality("low"))} />
      <Loop />
      <hemisphereLight args={earth ? ["#dceeff", "#8c7a5e", 1.2] : ["#d8a982", "#3a2018", 1.15]} />
      <directionalLight
        position={sun.toArray()}
        intensity={earth ? 3.0 : 3.2}
        color={earth ? "#fff4e0" : "#ffd7b0"}
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-camera-left={-200}
        shadow-camera-right={200}
        shadow-camera-top={200}
        shadow-camera-bottom={-200}
        shadow-camera-near={10}
        shadow-camera-far={12000}
        shadow-bias={-0.0004}
      />
      <World />
      {earth && <Tower />}
      <ShipPaths />
      <Ship />
      <ShipCameras />
      <Effects quality={quality} />
    </Canvas>
  );
}
