"use client";

import { useEffect } from "react";
import { useSolver } from "@/hooks/useSolver";
import { PAD_CAM_SECONDS } from "@/lib/camera";
import { useApp } from "@/lib/store";
import { landOrReset } from "@/lib/landOrReset";
import { Fallback, SceneErrorBoundary } from "./hud/Fallback";
import { Hud } from "./hud/Hud";
import { Booster } from "./scene/Booster";
import { Cameras } from "./scene/Cameras";
import { DragHandles } from "./scene/DragHandles";
import { FlightLoop } from "./scene/FlightLoop";
import { GlideCone } from "./scene/GlideCone";
import { Pad } from "./scene/Pad";
import { PlannedPath } from "./scene/PlannedPath";
import { Stage } from "./scene/Stage";
import { Terrain } from "./scene/Terrain";

const DEFAULT_GLIDE_SLOPE = (4 * Math.PI) / 180;

/** Client root: solver, 3D scene and HUD. */
export default function KestrelApp() {
  useSolver();
  const glideSlope = useApp((s) => s.vehicle?.glide_slope ?? DEFAULT_GLIDE_SLOPE);
  const blinking = useApp(
    (s) =>
      s.phase === "flying" &&
      !!s.flight &&
      s.flight.t > s.flight.response.trajectory.final_time - PAD_CAM_SECONDS,
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.code === "Space" && !(e.target instanceof HTMLInputElement)) {
        e.preventDefault();
        landOrReset();
      }
    };
    window.addEventListener("keydown", onKey);
    // Read-only handle for end-to-end tests and debugging.
    (window as unknown as { __kestrel: unknown }).__kestrel = { getState: useApp.getState };
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="app">
      <SceneErrorBoundary>
      <Stage>
        <FlightLoop />
        <Terrain />
        <Pad blinking={blinking} />
        <GlideCone glideSlope={glideSlope} />
        <PlannedPath />
        <Booster />
        <DragHandles />
        <Cameras />
      </Stage>
      </SceneErrorBoundary>
      <Hud />
      <Fallback />
    </div>
  );
}
