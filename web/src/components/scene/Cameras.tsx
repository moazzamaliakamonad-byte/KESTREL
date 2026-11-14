"use client";

import { OrbitControls } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useRef, useSyncExternalStore } from "react";
import * as THREE from "three";
import type { OrbitControls as OrbitControlsImpl } from "three-stdlib";
import { autoMode } from "@/lib/camera";
import { live } from "@/lib/live";
import { toScene } from "@/lib/playback";
import { useApp } from "@/lib/store";

const CHASE_OFFSET = new THREE.Vector3(-55, 22, 85);
const PAD_POSITION = new THREE.Vector3(-70, 7, 95);
/** Default orbit focus: middle of the example approach, seen from the side. */
const ORBIT_TARGET: [number, number, number] = [1000, 600, 0];
/** Initial camera position (side view of the whole approach). */
export const INITIAL_CAMERA: [number, number, number] = [700, 900, 3900];

function subscribeMotion(cb: () => void) {
  const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
  mq.addEventListener("change", cb);
  return () => mq.removeEventListener("change", cb);
}
const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Orbit (free), chase (follows the booster) and pad (low, looking up). */
export function Cameras() {
  const controls = useRef<OrbitControlsImpl>(null);
  const reduce = useSyncExternalStore(subscribeMotion, reducedMotion, () => false);
  const dragging = useApp((s) => s.dragging);
  const lookAt = useRef(new THREE.Vector3(...ORBIT_TARGET));
  const lastMode = useRef<string>("orbit");
  const lastPhase = useRef<string>("loading");

  useFrame(({ camera, size }, dt) => {
    // Test/debug probe: physics point → CSS pixel position on the canvas.
    const w = window as unknown as { __kestrel?: { screenOf?: unknown } };
    if (w.__kestrel) {
      w.__kestrel.screenOf = (p: [number, number, number]) => {
        const v = new THREE.Vector3(...toScene(p)).project(camera);
        return { x: ((v.x + 1) / 2) * size.width, y: ((1 - v.y) / 2) * size.height };
      };
    }
    const s = useApp.getState();
    const sample = live.sample;
    if (!sample) return;
    const mode = s.cameraOverride ?? autoMode(s.phase, s.flight?.nearTouchdown ?? false, reduce);
    const booster = new THREE.Vector3(...toScene(sample.position)).add(new THREE.Vector3(0, 6, 0));
    const k = 1 - Math.exp(-dt * 6);
    // Cinematic cuts: jump straight to the new shot when the mode changes.
    const cut = mode !== lastMode.current;
    lastMode.current = mode;

    if (controls.current) controls.current.enabled = mode === "orbit" && !dragging;

    if (mode === "chase") {
      camera.position.lerp(booster.clone().add(CHASE_OFFSET), cut ? 1 : k);
      lookAt.current.lerp(booster, cut ? 1 : Math.min(1, k * 2));
      camera.lookAt(lookAt.current);
    } else if (mode === "pad") {
      camera.position.lerp(PAD_POSITION, cut ? 1 : k);
      lookAt.current.lerp(booster, cut ? 1 : Math.min(1, k * 2));
      camera.lookAt(lookAt.current);
    } else if (controls.current) {
      if (s.phase === "editing" && lastPhase.current !== "editing" && lastPhase.current !== "loading") {
        // Back from a flight (Reset): return to the overview of the approach.
        camera.position.set(...INITIAL_CAMERA);
        controls.current.target.set(...ORBIT_TARGET);
      } else if (cut) {
        // Entering orbit from a flight shot: orbit around what we were looking at.
        controls.current.target.copy(lookAt.current);
      }
      // Orbit: the visitor is in charge while editing; after touchdown, ease
      // the target onto the landed booster to frame it.
      if (s.phase === "touchdown") controls.current.target.lerp(booster, k * 0.6);
      lookAt.current.copy(controls.current.target);
      controls.current.update();
    }
    lastPhase.current = s.phase;
  });

  return (
    <OrbitControls
      ref={controls}
      makeDefault
      target={ORBIT_TARGET}
      maxDistance={9000}
      minDistance={20}
      maxPolarAngle={Math.PI * 0.495}
      enableDamping
    />
  );
}
