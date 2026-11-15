"use client";

import { useFrame } from "@react-three/fiber";
import { useRef } from "react";
import * as THREE from "three";
import { flightAudio, SPEED_OF_SOUND } from "@/lib/ship/audio";
import { toScene } from "@/lib/ship/frames";
import { shipLive } from "@/lib/ship/live";
import { useShip } from "@/lib/ship/store";
import { shotOf } from "./Cameras";
import { launchLive } from "./Vehicle";

const at = new THREE.Vector3();

/**
 * Feeds the audio engine what a microphone at the camera would hear, every
 * frame, and fires the one-off sounds: the sonic boom when the vehicle
 * goes subsonic, the catch, the loss of the vehicle.
 */
export function Sound() {
  const prev = useRef<{ mach: number; outcome: string | null; t: number }>({ mach: 0, outcome: null, t: 0 });

  useFrame(({ camera }) => {
    const st = useShip.getState();
    flightAudio.enabled = st.view.sound;
    if (st.mode === "launch") {
      const ls = launchLive.state;
      const high = ls.phase === "staging" || ls.phase === "card";
      flightAudio.update({
        power: launchLive.active ? ls.power * (high ? 0.4 : 1.4) : 0,
        engines: launchLive.active && ls.power > 0.03 ? 13 : 0,
        distance: camera.position.distanceTo(launchLive.focus),
        onboard: false,
        q: 0,
        // At staging there is almost no air to carry sound to an outside camera.
        air: high ? 0.02 : 1,
        paused: false,
      });
      prev.current = { mach: 0, outcome: null, t: 0 };
      return;
    }
    const s = st.mode === "editing" ? null : shipLive.snapshot;
    const info = st.info[st.scenario];
    if (!s || !info) {
      flightAudio.update({ power: 0, engines: 0, distance: 1000, onboard: false, q: 0, air: 1, paused: true });
      return;
    }
    at.set(...toScene(s.position));
    const distance = camera.position.distanceTo(at);
    const shot = st.view.paused ? "orbit" : shotOf(st.view.camera, s, st.planet);
    const sea = st.planet === "earth" ? 1.225 : 0.02;
    flightAudio.update({
      power: s.thrust_mag / (info.ship.engine_max * info.ship.engines),
      engines: s.engines.filter((e) => e === "running").length,
      distance,
      onboard: shot === "onboard",
      q: s.q_dyn,
      // Mars' thin air carries sound poorly: quieter, but not silent.
      air: st.planet === "earth" ? s.density / sea : 0.12 * (s.density / sea),
      paused: st.view.paused || st.mode === "done",
    });

    const p = prev.current;
    if (s.t > p.t) {
      // Going subsonic: the boom reaches a camera outside the vehicle later.
      if (p.mach >= 1 && s.mach < 1 && shot !== "onboard" && st.planet === "earth") flightAudio.sonicBoom(distance);
      if (s.outcome && !p.outcome) {
        const after = shot === "onboard" ? 0 : distance / SPEED_OF_SOUND;
        if (s.outcome === "crashed") flightAudio.explosion(after);
        else flightAudio.clunk(after);
      }
    }
    prev.current = { mach: s.mach, outcome: s.outcome, t: s.t };
  });
  return null;
}
