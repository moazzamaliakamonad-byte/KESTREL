"use client";

import { OrbitControls } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useRef } from "react";
import * as THREE from "three";
import type { OrbitControls as OrbitControlsImpl } from "three-stdlib";
import { quatToScene, toScene, type Vec3 } from "@/lib/ship/frames";
import { shipLive } from "@/lib/ship/live";
import { useShip, type CameraMode } from "@/lib/ship/store";
import type { ShipSnapshot } from "@/lib/ship/types";
import { launchLive } from "./Vehicle";

/** A concrete shot (the director picks among these). */
export type Shot = "pad" | "launch" | "chase" | "track" | "onboard" | "tower" | "orbit" | "circle";

/** The director's cut: which shot suits this moment of the flight. */
export function directorShot(s: ShipSnapshot | null, planet: "earth" | "mars"): Shot {
  if (!s) return "chase";
  if (s.outcome) return "circle";
  const z = s.position[2];
  switch (s.phase) {
    case "terminal":
      return planet === "earth" ? "tower" : "chase";
    case "landing-burn":
      return z > 900 || planet === "mars" ? "chase" : "track";
    case "startup":
    case "flip":
      return "chase";
    default: {
      // Unpowered: cycle wide chase, the long lens from the ground, and the onboard view.
      const phase = s.t % 24;
      if (z > 30_000) return phase < 14 ? "chase" : "onboard";
      return phase < 10 ? "chase" : phase < 17 ? "track" : "onboard";
    }
  }
}

export function shotOf(mode: CameraMode, s: ShipSnapshot | null, planet: "earth" | "mars"): Shot {
  return mode === "director" ? directorShot(s, planet) : mode;
}

const UP = new THREE.Vector3(0, 1, 0);
const v = new THREE.Vector3();
const q = new THREE.Quaternion();

/**
 * Cinematic cameras. Director (automatic cuts), chase, a ground tracking
 * camera with a long lens, an onboard view looking down the vehicle, the
 * tower's view, and a free orbit. While paused the view is always a free
 * orbit around the vehicle, from wherever the camera was.
 */
export function Cameras() {
  const controls = useRef<OrbitControlsImpl>(null);
  const lookAt = useRef(new THREE.Vector3(0, 80, 0));
  const last = useRef<Shot | "">("");
  const lastTarget = useRef(new THREE.Vector3());
  const lastLaunchPhase = useRef("");
  /** The camera's position before shake was added (so shake never accumulates). */
  const steady = useRef(new THREE.Vector3());
  const shaken = useRef(false);

  useFrame(({ camera, clock }, dt) => {
    const cam = camera as THREE.PerspectiveCamera;
    const st = useShip.getState();
    const editing = st.mode === "editing";
    const launching = st.mode === "launch";
    const snap = editing || launching ? null : shipLive.snapshot;
    // Undo last frame's shake before moving the camera.
    if (shaken.current) cam.position.copy(steady.current);
    const p: Vec3 = snap?.position ?? st.start.position;
    const ship = v.set(...toScene(p)).clone();
    const info = st.info[st.scenario];
    const length = info?.ship.length ?? 60;
    const shot: Shot = editing ? "pad" : launching ? "launch" : st.view.paused ? "orbit" : shotOf(st.view.camera, snap, st.planet);
    const cut = shot !== last.current;
    const from = last.current;
    last.current = shot;
    const k = 1 - Math.exp(-dt * 4);
    const free = shot === "orbit" || shot === "pad";
    if (controls.current) {
      controls.current.enabled = free;
      controls.current.autoRotate = shot === "pad";
      // On the pad the camera stays above ground; in the air it may look from anywhere.
      controls.current.maxPolarAngle = shot === "pad" ? Math.PI * 0.497 : Math.PI * 0.98;
    }
    let fov = 42;
    cam.up.copy(UP);

    let shake = 0;
    if (shot === "launch") {
      const ls = launchLive.state;
      const f = launchLive.focus;
      const high = ls.phase === "staging" || ls.phase === "card";
      const phaseCut = cut || (high ? "high" : "low") !== lastLaunchPhase.current;
      lastLaunchPhase.current = high ? "high" : "low";
      if (launchLive.active && high) {
        // Alongside the stack at the edge of space, drifting slowly around it.
        const a = 0.9 + ls.t * 0.03;
        // On the sunlit side, far enough to hold both stages as they part.
        const d = 250 + 14 * ls.apart;
        cam.position.copy(f).add(new THREE.Vector3(Math.sin(a) * d, 40, Math.cos(a) * d));
        lookAt.current.copy(f);
        fov = 36;
        shake = 0.05;
      } else if (launchLive.active) {
        // Low beside the pad, tilting up to follow the climb.
        cam.position.lerp(new THREE.Vector3(150, 9, -205), phaseCut ? 1 : k * 0.5);
        lookAt.current.lerp(f, phaseCut ? 1 : Math.min(1, 3 * k));
        fov = 46 + Math.min(16, ls.height * 0.03);
        shake = ls.power * 0.7 * THREE.MathUtils.clamp(1 - ls.height / 1200, 0.25, 1);
      }
      cam.lookAt(lookAt.current);
    } else if (shot === "pad") {
      if (cut && controls.current) {
        controls.current.target.set(0, st.planet === "earth" ? 78 : 30, 0);
        // From the land side, so the sea is behind the tower.
        cam.position.set(190, st.planet === "earth" ? 46 : 40, -270);
      }
      controls.current?.update();
    } else if (shot === "orbit") {
      if (controls.current) {
        if (cut) {
          controls.current.target.copy(ship);
          // Coming from a far or onboard shot, step back to a sensible distance.
          const d = cam.position.distanceTo(ship);
          if (d > 900 || d < 30 || from === "onboard") cam.position.copy(ship).add(new THREE.Vector3(0.62, 0.2, 0.76).multiplyScalar(3.4 * length));
        } else {
          // Follow the vehicle without changing the view of it.
          cam.position.add(ship.clone().sub(lastTarget.current));
          controls.current.target.copy(ship);
        }
        controls.current.update();
      }
    } else if (shot === "chase" || shot === "circle") {
      const speed = snap ? Math.hypot(...snap.velocity) : 0;
      const d = 3.2 * length + Math.min(520, speed * 0.75);
      const a = shot === "circle" ? clock.elapsedTime * 0.12 : 0.9;
      const offset = new THREE.Vector3(Math.sin(a) * 0.82, shot === "circle" ? 0.12 : 0.24, Math.cos(a) * 0.82).multiplyScalar(d);
      const want = ship.clone().add(offset);
      want.y = Math.max(want.y, 8);
      cam.position.lerp(want, cut ? 1 : k);
      lookAt.current.lerp(ship, cut ? 1 : Math.min(1, 2.5 * k));
      cam.lookAt(lookAt.current);
      fov = 40;
    } else if (shot === "track") {
      // A long lens on the ground, as launch tracking cameras are.
      const at = st.planet === "earth" ? new THREE.Vector3(330, 4, 520) : new THREE.Vector3(260, 14, 380);
      cam.position.lerp(at, cut ? 1 : k);
      lookAt.current.lerp(ship, cut ? 1 : Math.min(1, 3 * k));
      const dist = cam.position.distanceTo(ship);
      fov = THREE.MathUtils.clamp(THREE.MathUtils.radToDeg(2 * Math.atan((1.25 * length) / dist)), 0.8, 46);
      // The operator's hand: a little wander that grows with the zoom.
      const wobble = dist * 0.0006 * Math.min(1, 12 / fov);
      lookAt.current.x += Math.sin(clock.elapsedTime * 1.7) * wobble;
      lookAt.current.y += Math.sin(clock.elapsedTime * 2.3 + 1) * wobble;
      cam.lookAt(lookAt.current);
    } else if (shot === "onboard") {
      // Fixed to the vehicle near its top, looking down the hull at the engines.
      q.set(...quatToScene(snap?.quat ?? [1, 0, 0, 0]));
      const r = (info?.ship.radius ?? 4.5) + 1.6;
      const top = length - (info?.ship.cm_height ?? 24) - 9;
      const side = st.vehicle === "ship" ? -1 : 1;
      cam.position.copy(ship).add(new THREE.Vector3(side * r, top, 0.8).applyQuaternion(q));
      const target = ship.clone().add(new THREE.Vector3(side * r * 0.35, -(info?.ship.cm_height ?? 24) - 30, 0).applyQuaternion(q));
      cam.up.set(side, 0, 0).applyQuaternion(q);
      cam.lookAt(target);
      lookAt.current.copy(target);
      fov = 68;
    } else if (shot === "tower") {
      const c = info?.catch;
      const at = c ? new THREE.Vector3(c.mast_x + 7, c.tower_top - 6, 16) : new THREE.Vector3(60, 30, 80);
      cam.position.lerp(at, cut ? 1 : k);
      lookAt.current.lerp(ship, cut ? 1 : Math.min(1, 3 * k));
      cam.lookAt(lookAt.current);
      const dist = cam.position.distanceTo(ship);
      fov = THREE.MathUtils.clamp(THREE.MathUtils.radToDeg(2 * Math.atan((1.1 * length) / dist)), 14, 62);
    }

    // Shake. A camera on the ground near a big burn is rattled bodily.
    // The onboard camera is bolted to the hull: it cannot move against
    // the vehicle, it only buzzes, a fraction of a degree at high
    // frequency, with the engines and the airflow.
    let buzz = 0;
    if (snap && !free && !st.view.paused) {
      const power = Math.min(1, snap.thrust_mag / 2.5e7);
      if (shot === "onboard") buzz = 0.0007 * power + 0.0012 * Math.min(1, snap.q_dyn / 60_000);
      else shake = power * 0.9 * THREE.MathUtils.clamp(400 / Math.max(60, cam.position.distanceTo(ship)), 0, 1);
    }
    steady.current.copy(cam.position);
    shaken.current = shake > 0.001;
    const t = clock.elapsedTime;
    if (shaken.current) {
      // Two incommensurate frequencies per axis: a rattle, not a wobble.
      cam.position.x += (Math.sin(t * 71) * 0.6 + Math.sin(t * 113 + 1) * 0.4) * shake;
      cam.position.y += (Math.sin(t * 83 + 2) * 0.6 + Math.sin(t * 127) * 0.4) * shake;
      cam.position.z += Math.sin(t * 97 + 4) * 0.5 * shake;
      cam.rotation.z += Math.sin(t * 61) * 0.0012 * shake;
    }
    if (buzz > 0) {
      // (The orientation is set afresh by lookAt every frame, so this never accumulates.)
      cam.rotateX((Math.sin(t * 211) * 0.6 + Math.sin(t * 337 + 1) * 0.4) * buzz);
      cam.rotateY((Math.sin(t * 263 + 2) * 0.6 + Math.sin(t * 389) * 0.4) * buzz);
    }

    lastTarget.current.copy(ship);
    if (!free && Math.abs(cam.fov - fov) > 0.01) {
      cam.fov = cut ? fov : cam.fov + (fov - cam.fov) * Math.min(1, dt * 3);
      cam.updateProjectionMatrix();
    } else if (free && Math.abs(cam.fov - 42) > 0.01) {
      cam.fov = 42;
      cam.updateProjectionMatrix();
    }
  });

  return (
    <OrbitControls
      ref={controls}
      makeDefault
      enableDamping
      autoRotateSpeed={0.35}
      maxDistance={60000}
      minDistance={25}
      maxPolarAngle={Math.PI * 0.497}
    />
  );
}
