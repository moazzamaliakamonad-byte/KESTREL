"use client";

import { Line } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import { sampleAt, toScene } from "@/lib/playback";
import { useApp } from "@/lib/store";
import type { PlanResponse, Vec3 } from "@/lib/types";

const SAMPLES = 240;
const START_COLOR = new THREE.Color("#7fe3ff").multiplyScalar(2.2);
const END_COLOR = new THREE.Color("#ffffff").multiplyScalar(2.6);
const DANGER = new THREE.Color("#ff5a4f").multiplyScalar(2.4);

function feasiblePoints(response: PlanResponse & { ok: true }) {
  const { trajectory, vehicle } = response;
  const points: Vec3[] = [];
  const colors: [number, number, number][] = [];
  for (let i = 0; i <= SAMPLES; i++) {
    const s = sampleAt(trajectory, vehicle, (trajectory.final_time * i) / SAMPLES);
    points.push(toScene(s.position));
    const c = START_COLOR.clone().lerp(END_COLOR, i / SAMPLES);
    colors.push([c.r, c.g, c.b]);
  }
  return { points, colors };
}

/**
 * The plan as a glowing line: cyan → white over time. Dimmed while a newer
 * plan is being solved; a red pulsing dashed line toward the pad when no
 * landing exists from the current start.
 */
export function PlannedPath() {
  const phase = useApp((s) => s.phase);
  const preview = useApp((s) => s.preview);
  const flight = useApp((s) => s.flight);
  const pending = useApp((s) => s.requestedAt !== null);
  const start = useApp((s) => s.start);
  const danger = useRef<THREE.Material & { opacity: number }>(null);

  const response = phase === "editing" ? preview?.response : flight?.response;
  const feasible = useMemo(
    () => (response && response.ok ? feasiblePoints(response) : null),
    [response],
  );

  useFrame(({ clock }) => {
    if (danger.current) danger.current.opacity = 0.55 + 0.45 * Math.sin(clock.elapsedTime * 5);
  });

  if (phase === "loading" || phase === "failed") return null;

  if (feasible) {
    return (
      <Line
        points={feasible.points}
        vertexColors={feasible.colors}
        lineWidth={3}
        transparent
        opacity={pending && phase === "editing" ? 0.35 : 0.95}
        toneMapped={false}
      />
    );
  }

  if (response && !response.ok && phase === "editing") {
    return (
      <Line
        ref={(l) => {
          danger.current = (l?.material as THREE.Material & { opacity: number }) ?? null;
        }}
        points={[toScene(start.position), [0, 0, 0]]}
        color={DANGER}
        lineWidth={3}
        dashed
        dashSize={60}
        gapSize={40}
        transparent
        toneMapped={false}
      />
    );
  }
  return null;
}
