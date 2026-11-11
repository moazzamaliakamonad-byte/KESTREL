"use client";

// TEMPORARY (Task 7 visual check): loops the solved example landing with a
// simple following camera. Replaced by the real flight loop in Task 8.

import { useFrame } from "@react-three/fiber";
import { useRef } from "react";
import * as THREE from "three";
import fixture from "@/lib/__fixtures__/example-response.json";
import { live } from "@/lib/live";
import { sampleAt, toScene } from "@/lib/playback";
import type { PlanResponse } from "@/lib/types";

const response = fixture as PlanResponse;

export function DemoLoop() {
  const t = useRef(0);
  const look = useRef(new THREE.Vector3());
  useFrame(({ camera }, dt) => {
    if (!response.ok) return;
    const { trajectory, vehicle } = response;
    const params = new URLSearchParams(window.location.search);
    const frozen = params.get("t");
    t.current = frozen !== null ? Number(frozen) : (t.current + dt) % (trajectory.final_time + 3);
    live.maxThrust = vehicle.max_thrust;
    live.sample = sampleAt(trajectory, vehicle, t.current);
    const p = new THREE.Vector3(...toScene(live.sample.position));
    const want = p.clone().add(new THREE.Vector3(-40, 14, 70));
    camera.position.lerp(want, frozen !== null ? 1 : 0.08);
    look.current.lerp(p.clone().add(new THREE.Vector3(0, 4, 0)), frozen !== null ? 1 : 0.15);
    camera.lookAt(look.current);
  });
  return null;
}
