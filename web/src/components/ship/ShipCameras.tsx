"use client";

import { OrbitControls } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useRef } from "react";
import * as THREE from "three";
import type { OrbitControls as OrbitControlsImpl } from "three-stdlib";
import { toScene, type Vec3 } from "@/lib/ship/frames";
import { shipLive } from "@/lib/ship/live";
import { useShip } from "@/lib/ship/store";

/** Chase (behind and beside the ship), tower (low, near the mount, looking
 * up) and orbit (free; the default while editing). Distances suit a 52 m
 * ship and a 145 m tower. */
export function ShipCameras() {
  const controls = useRef<OrbitControlsImpl>(null);
  const lookAt = useRef(new THREE.Vector3(0, 300, 0));
  const last = useRef("");

  useFrame(({ camera }, dt) => {
    const st = useShip.getState();
    const snap = st.mode === "editing" ? null : shipLive.snapshot;
    const p: Vec3 = snap?.position ?? st.start.position;
    const ship = new THREE.Vector3(...toScene(p));
    const mode = st.mode === "editing" ? "orbit" : st.camera;
    const cut = mode !== last.current;
    last.current = mode;
    const k = 1 - Math.exp(-dt * 4);
    if (controls.current) controls.current.enabled = mode === "orbit";

    if (mode === "chase") {
      // Pull back with speed so fast phases stay in frame.
      const speed = snap ? Math.hypot(...snap.velocity) : 0;
      const d = 170 + Math.min(450, speed * 3);
      const offset = new THREE.Vector3(0.55 * d, 0.25 * d, 0.8 * d);
      camera.position.lerp(ship.clone().add(offset), cut ? 1 : k);
      lookAt.current.lerp(ship, cut ? 1 : Math.min(1, 2 * k));
      camera.lookAt(lookAt.current);
    } else if (mode === "tower") {
      const at = st.scenario === "earth" ? new THREE.Vector3(90, 18, 140) : new THREE.Vector3(120, 12, 160);
      camera.position.lerp(at, cut ? 1 : k);
      lookAt.current.lerp(ship, cut ? 1 : Math.min(1, 2 * k));
      camera.lookAt(lookAt.current);
    } else if (controls.current) {
      if (cut) {
        // Editing: frame the whole descent, start to tower, from the side.
        const h = p[2];
        const mid = new THREE.Vector3(p[0] / 2, h * 0.45, 0);
        controls.current.target.copy(mid);
        camera.position.set(mid.x + 0.35 * h, 0.12 * h + 150, 1.15 * h + 800);
      }
      controls.current.update();
    }
  });

  return (
    <OrbitControls
      ref={controls}
      makeDefault
      maxDistance={30000}
      minDistance={40}
      maxPolarAngle={Math.PI * 0.495}
    />
  );
}
