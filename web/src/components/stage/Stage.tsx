"use client";

import { PerformanceMonitor } from "@react-three/drei";
import { Canvas, useFrame } from "@react-three/fiber";
import { useState } from "react";
import { shipFrame } from "@/lib/ship/runtime";
import { useShip } from "@/lib/ship/store";
import { Analysis } from "./Analysis";
import { Cameras } from "./Cameras";
import { Paths, Smoke } from "./Fx";
import { Post, type Quality } from "./Post";
import { Sound } from "./Sound";
import { ParkedBooster, Stack, Vehicle } from "./Vehicle";
import { World } from "./World";

/** Steps the closed loop once per rendered frame while flying. */
function Loop() {
  useFrame((_, dt) => {
    if (useShip.getState().mode === "flying") shipFrame(dt);
  });
  return null;
}

/** The 3D view: world, tower, the full stack before a flight, the vehicle, effects, cameras. */
export function Stage() {
  const planet = useShip((s) => s.planet);
  const [dpr, setDpr] = useState(2);
  const [quality, setQuality] = useState<Quality>("high");
  return (
    <Canvas
      shadows
      flat
      dpr={[1, dpr]}
      gl={{ antialias: false, powerPreference: "high-performance", logarithmicDepthBuffer: true }}
      camera={{ fov: 42, near: 1, far: 3_000_000, position: [190, 46, -270] }}
      key={planet}
    >
      <PerformanceMonitor onDecline={() => (dpr > 1 ? setDpr(1) : setQuality("low"))} />
      <Loop />
      <World />
      <Stack />
      <ParkedBooster />
      <Paths />
      <Vehicle />
      <Smoke />
      <Analysis />
      <Cameras />
      <Sound />
      <Post quality={quality} />
    </Canvas>
  );
}
