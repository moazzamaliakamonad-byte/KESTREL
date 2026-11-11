"use client";

import { useEffect, useRef, useState } from "react";
import { useAnimationFrame } from "@/hooks/useAnimationFrame";
import type { CameraMode } from "@/lib/camera";
import { live } from "@/lib/live";
import { createRumble } from "@/lib/sound";
import { canLand, type Speed } from "@/lib/state";
import { useApp } from "@/lib/store";
import { landOrReset } from "@/lib/landOrReset";
import s from "./hud.module.css";

const SPEEDS: Speed[] = [1, 2, 4];
const CAMERAS: { mode: CameraMode | null; label: string }[] = [
  { mode: null, label: "Auto" },
  { mode: "chase", label: "Chase" },
  { mode: "pad", label: "Pad" },
  { mode: "orbit", label: "Free" },
];

/** Land / Reset, playback speed, camera and sound. */
export function Controls() {
  const phase = useApp((st) => st.phase);
  const ready = useApp(canLand);
  const speed = useApp((st) => st.speed);
  const camera = useApp((st) => st.cameraOverride);
  const [sound, setSound] = useState(false);
  const rumble = useRef<ReturnType<typeof createRumble> | null>(null);

  useEffect(() => {
    if (!sound) return;
    rumble.current = createRumble();
    return () => {
      rumble.current?.dispose();
      rumble.current = null;
    };
  }, [sound]);

  useAnimationFrame(() => {
    const smp = live.sample;
    rumble.current?.setLevel(smp && useApp.getState().phase === "flying" ? smp.thrustMag / live.maxThrust : 0);
  });

  const flying = phase === "flying" || phase === "touchdown";
  const { dispatch, setCameraOverride } = useApp.getState();

  return (
    <nav className={`${s.controls} ${s.glass}`} aria-label="Controls">
      <button
        className={s.land}
        onClick={landOrReset}
        disabled={!flying && !ready}
        title="Space"
      >
        {flying ? "Reset" : "Land"}
      </button>
      <div className={s.group} role="group" aria-label="Playback speed">
        {SPEEDS.map((x) => (
          <button
            key={x}
            className={s.seg}
            aria-pressed={speed === x}
            onClick={() => dispatch({ type: "setSpeed", speed: x })}
          >
            ×{x}
          </button>
        ))}
      </div>
      <div className={s.group} role="group" aria-label="Camera">
        {CAMERAS.map((c) => (
          <button
            key={c.label}
            className={s.seg}
            aria-pressed={camera === c.mode}
            onClick={() => setCameraOverride(c.mode)}
          >
            {c.label}
          </button>
        ))}
      </div>
      <button className={s.seg} aria-pressed={sound} onClick={() => setSound((v) => !v)}>
        {sound ? "Sound on" : "Sound off"}
      </button>
    </nav>
  );
}
