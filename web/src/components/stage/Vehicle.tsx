"use client";

import { shown } from "@/lib/ship/shown";
import { Billboard } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useRef } from "react";
import * as THREE from "three";
import { HEATING_OFF } from "@/lib/ship/commentary";
import { quatToScene, rotateInv, startAttitude, toScene, type PQuat, type Vec3 } from "@/lib/ship/frames";
import { launchState, type LaunchState } from "@/lib/ship/launch";
import { useShip } from "@/lib/ship/store";
import type { EngineState } from "@/lib/ship/types";
import { BlueprintSkin, skin } from "./Blueprint";
import { BOOSTER, BoosterModel, ShipModel, type VehiclePose } from "./Vehicles";
import { MOUNT_TOP } from "./World";

/** Heating (W/m²) at which the glow is fully on. */
const HEAT_FULL = 160_000;

/** 0–1 glow for a heating estimate. */
export function heatGlow(flux: number): number {
  return THREE.MathUtils.clamp((flux - HEATING_OFF * 0.6) / (HEAT_FULL - HEATING_OFF * 0.6), 0, 1);
}

/**
 * What the opening sequence is doing now, for the cameras, smoke and sound
 * (written by `Stack` every frame; `phase` "over" when no sequence plays).
 */
export const launchLive: { state: LaunchState; focus: THREE.Vector3; active: boolean } = {
  state: launchState(1e9),
  focus: new THREE.Vector3(0, 80, 0),
  active: false,
};

/**
 * The vehicle being flown: placed from the simulator's state every frame.
 * Before a flight it waits at the chosen start, in its start attitude.
 */
export function Vehicle() {
  const vehicle = useShip((s) => s.vehicle);
  const root = useRef<THREE.Group>(null);
  const halo = useRef<THREE.Group>(null);
  const pose = useRef<VehiclePose | null>(null);

  useFrame(({ camera }) => {
    const g = root.current;
    if (!g) return;
    const st = useShip.getState();
    const flying = st.mode === "flying" || st.mode === "done";
    const s = flying ? shown() : null;
    const position: Vec3 = s?.position ?? st.start.position;
    const q: PQuat = s?.quat ?? startAttitude(st.vehicle, st.start.velocity);
    g.position.set(...toScene(position));
    g.quaternion.set(...quatToScene(q));
    // During the opening sequence the flight has not begun: nothing waits in the sky.
    g.visible = st.mode !== "launch";

    if (s) {
      const running = s.engines.filter((e) => e === "running").length;
      const perEngine = st.info[st.scenario]?.ship.engine_max ?? 2.45e6;
      const air: Vec3 = [s.velocity[0] - s.wind[0], s.velocity[1] - s.wind[1], s.velocity[2] - s.wind[2]];
      const speed = Math.hypot(...air) || 1;
      const flow = rotateInv(q, [air[0] / speed, air[1] / speed, air[2] / speed]);
      pose.current = {
        surfaces: s.flaps,
        thrust: s.thrust,
        engines: s.engines,
        throttle: running > 0 ? Math.min(1, s.thrust_mag / running / perEngine) : 0,
        rcs: s.rcs,
        heat: heatGlow(s.heat_flux),
        flowBody: flow,
        vacuum: THREE.MathUtils.clamp(1 - s.density / 1.225, 0, 1),
      };
    } else {
      pose.current = null;
    }

    // Far away the vehicle is a few pixels: a ring that keeps its size marks it.
    if (halo.current) {
      const dist = camera.position.distanceTo(g.position);
      const fov = (camera as THREE.PerspectiveCamera).fov ?? 45;
      const half = Math.tan((fov * Math.PI) / 360);
      halo.current.visible = g.visible && !skin.on && 70 / (dist * half) < 0.02;
      halo.current.position.copy(g.position);
      halo.current.scale.setScalar(dist * half * 0.05);
    }
  });

  return (
    <>
      <group ref={halo} visible={false}>
        <Billboard>
          <mesh>
            <ringGeometry args={[0.86, 1, 64]} />
            <meshBasicMaterial color="#8fe9ff" transparent opacity={0.9} depthTest={false} fog={false} />
          </mesh>
        </Billboard>
      </group>
      <group ref={root}>{vehicle === "booster" ? <BoosterModel pose={pose} /> : <ShipModel pose={pose} />}</group>
      {/* Keyed by vehicle: the meshes it swapped belong to that model. */}
      <BlueprintSkin key={vehicle} root={root} />
    </>
  );
}

/** A pose with the first `n` engines running. */
function firing(n: number, throttle: number, vacuum: number): VehiclePose {
  return {
    surfaces: [0.5, 0.5, 0.5, 0.5],
    thrust: [0, 0, 1e7],
    engines: Array.from({ length: 13 }, (_, i): EngineState => (i < n ? "running" : "off")),
    throttle,
    rcs: [],
    heat: 0,
    flowBody: [0, 0, -1],
    vacuum,
  };
}

const BOOSTER_CM = MOUNT_TOP + BOOSTER.cm;
/** The ship sits on the hot-stage ring, 1.8 m above the booster's tank. */
const SHIP_CM = MOUNT_TOP + BOOSTER.length + 1.8 + 24;
/** Pitch of the stack at staging, rad from vertical (it is flying mostly sideways by then). */
const STAGING_PITCH = -1.05;

/**
 * The full stack: booster with the ship on top. It stands on the launch
 * mount before a flight; in the opening sequence it lifts off and, high
 * up, the two stages separate: two vehicles from then on. The sequence is
 * an illustration (ascent is not simulated). Once the simulated return
 * begins the stack is gone: the vehicle that comes back is one of the two.
 */
export function Stack() {
  const show = useShip((s) => !s.parked && (s.mode === "editing" || (s.mode === "launch" && s.intro === "stack")));
  const root = useRef<THREE.Group>(null);
  const booster = useRef<THREE.Group>(null);
  const ship = useRef<THREE.Group>(null);
  const boosterPose = useRef<VehiclePose | null>(null);
  const shipPose = useRef<VehiclePose | null>(null);

  useFrame(() => {
    const st = useShip.getState();
    const playing = st.mode === "launch" && st.intro === "stack";
    const ls = playing ? launchState((performance.now() - st.launchAt) / 1000, true) : launchState(1e9);
    launchLive.state = ls;
    launchLive.active = playing && ls.phase !== "over";
    const g = root.current;
    if (!g || !booster.current || !ship.current) return;
    const high = ls.phase === "staging" || ls.phase === "card";
    booster.current.position.set(0, BOOSTER_CM, 0);
    booster.current.rotation.set(0, 0, 0);
    ship.current.position.set(0, SHIP_CM, 0);
    if (!launchLive.active) {
      g.position.set(0, 0, 0);
      g.rotation.set(0, 0, 0);
      boosterPose.current = null;
      shipPose.current = null;
      launchLive.focus.set(0, 80, 0);
      return;
    }
    if (!high) {
      // On the pad, then climbing with a slow pitch-over toward the sea.
      g.position.set(0, ls.height, 0);
      g.rotation.set(0, 0, -Math.min(0.2, ls.height * 0.0004));
      boosterPose.current = ls.power > 0.03 ? firing(13, ls.power, 0) : null;
      shipPose.current = null;
      launchLive.focus.set(0, ls.height + 80, 0);
      return;
    }
    // The edge of space: engines throttle down, the ship lights its own
    // and pulls away while the booster starts to turn for its return.
    g.position.set(2000, ls.height, 0);
    g.rotation.set(0, 0, STAGING_PITCH);
    const a = ls.apart;
    const gap = 2.2 * a * a + 3 * a;
    ship.current.position.y = SHIP_CM + gap;
    booster.current.position.x = -1.5 * a * a;
    booster.current.rotation.z = 0.035 * a * a;
    boosterPose.current = firing(a > 0 ? 3 : 13, a > 0 ? 0.5 : 0.55, 1);
    shipPose.current = a > 0 ? firing(3, 1, 1) : null;
    // Look between the two as they part.
    launchLive.focus.set(0, MOUNT_TOP + BOOSTER.length + gap / 2, 0).applyEuler(g.rotation).add(g.position);
  });

  if (!show) return null;
  return (
    <group ref={root}>
      <group ref={booster} position-y={BOOSTER_CM}>
        <BoosterModel pose={boosterPose} />
      </group>
      <group ref={ship} position-y={SHIP_CM} rotation-y={Math.PI}>
        <ShipModel pose={shipPose} />
      </group>
    </group>
  );
}

const NO_POSE = { current: null };

/** The caught booster, set down on a transport stand beside the pad while the ship returns. */
export function ParkedBooster() {
  const parked = useShip((s) => s.parked);
  const g = useRef<THREE.Group>(null);
  // (The blueprint shows the flying vehicle alone.)
  useFrame(() => {
    if (g.current) g.current.visible = !skin.on;
  });
  if (!parked) return null;
  return (
    <group ref={g} position={[-95, 0, 105]}>
      <mesh position-y={3} castShadow receiveShadow>
        <cylinderGeometry args={[7, 8.5, 6, 8]} />
        <meshStandardMaterial color="#4a4d52" metalness={0.5} roughness={0.7} />
      </mesh>
      <group position-y={6 + BOOSTER.cm}>
        <BoosterModel pose={NO_POSE} />
      </group>
    </group>
  );
}
