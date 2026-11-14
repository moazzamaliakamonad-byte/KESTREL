"use client";

import { Billboard } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import { live } from "@/lib/live";
import { toScene } from "@/lib/playback";

/** Most trail points kept (10 per second: over 6 minutes of flight). */
const MAX_POINTS = 4000;
const TRAIL_COLOR = new THREE.Color("#ffb35c").multiplyScalar(1.6);
const NAV_COLOR = new THREE.Color("#ff6bd6").multiplyScalar(1.8);

/**
 * Where the booster has actually flown (orange), next to the plan (cyan).
 * The magenta ring marks where the sensors say it is; with noise on, it
 * wanders around the booster.
 */
export function Trail() {
  const lineObject = useMemo(() => {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(MAX_POINTS * 3), 3));
    geometry.setDrawRange(0, 0);
    const material = new THREE.LineBasicMaterial({
      color: TRAIL_COLOR,
      transparent: true,
      opacity: 0.85,
      toneMapped: false,
    });
    return new THREE.Line(geometry, material);
  }, []);
  const lineRef = useRef<THREE.Line>(null);
  const written = useRef(0);
  const nav = useRef<THREE.Group>(null);

  useFrame(({ camera }) => {
    const line = lineRef.current;
    if (!line) return;
    const trail = live.trail;
    if (trail.length < written.current) written.current = 0; // new flight
    const n = Math.min(trail.length, MAX_POINTS);
    const attr = line.geometry.getAttribute("position") as THREE.BufferAttribute;
    for (let i = written.current; i < n; i++) {
      attr.setXYZ(i, ...toScene(trail[i].position));
    }
    if (n !== written.current) {
      attr.needsUpdate = true;
      line.geometry.setDrawRange(0, n);
      written.current = n;
    }
    line.visible = n > 1;

    const g = nav.current;
    const snap = live.snapshot;
    if (g) {
      const off = !snap || snap.landed;
      g.visible = !off;
      if (snap && !off) {
        g.position.set(...toScene(snap.nav_position));
        g.scale.setScalar(camera.position.distanceTo(g.position) * 0.01);
      }
    }
  });

  return (
    <group>
      <primitive ref={lineRef} object={lineObject} frustumCulled={false} />
      <group ref={nav}>
        <Billboard>
          <mesh>
            <ringGeometry args={[1.0, 1.25, 32]} />
            <meshBasicMaterial color={NAV_COLOR} transparent opacity={0.9} depthTest={false} toneMapped={false} />
          </mesh>
        </Billboard>
      </group>
    </group>
  );
}
