"use client";

import { PerformanceMonitor } from "@react-three/drei";
import { Canvas, useFrame } from "@react-three/fiber";
import { useRef, useState } from "react";
import { shipLive } from "@/lib/ship/live";
import { shipFrame } from "@/lib/ship/runtime";
import { useShip } from "@/lib/ship/store";
import { Analysis } from "./Analysis";
import { SkinGuard } from "./Blueprint";
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

/** Recent solves slower than this (median, ms) and the picture gives way. */
export const SLOW_SOLVE_MS = 250;

/**
 * Guidance first: on a laptop the renderer and the planner share one power
 * and memory budget, and a busy picture made plans three to four times
 * slower. When recent plans are slow, step the picture down (pixel ratio
 * 1, then no bloom) until they are not.
 */
function GiveWay({ onSlow }: { onSlow: () => void }) {
  const last = useRef(0);
  useFrame(({ clock }) => {
    if (clock.elapsedTime - last.current < 3) return;
    last.current = clock.elapsedTime;
    const recent = shipLive.solveMs.slice(-5);
    if (recent.length < 3) return;
    const median = [...recent].sort((a, b) => a - b)[Math.floor(recent.length / 2)];
    if (median > SLOW_SOLVE_MS) onSlow();
  });
  return null;
}

/** The 3D view: world, tower, the full stack before a flight, the vehicle, effects, cameras. */
export function Stage() {
  const [dpr, setDpr] = useState(1.5);
  const [quality, setQuality] = useState<Quality>("high");
  return (
    <Canvas
      shadows
      flat
      dpr={[1, dpr]}
      gl={{ antialias: false, powerPreference: "high-performance", logarithmicDepthBuffer: true }}
      camera={{ fov: 42, near: 1, far: 3_000_000, position: [190, 46, -270] }}
    >
      <PerformanceMonitor onDecline={() => (dpr > 1 ? setDpr(1) : setQuality("low"))} />
      <GiveWay onSlow={() => (dpr > 1 ? setDpr(1) : setQuality("low"))} />
      <Loop />
      <SkinGuard />
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
