"use client";

import { useEffect } from "react";
import { SceneErrorBoundary } from "@/components/hud/Fallback";
import { shipLive } from "@/lib/ship/live";
import { useShip } from "@/lib/ship/store";
import { Console } from "./console/Console";
import { useEngine } from "./console/useEngine";
import { Stage } from "./stage/Stage";

/** The page: a Starship-class ship or booster flown back to the tower (or onto Mars), with a console around it. */
export default function App() {
  useEngine();
  useEffect(() => {
    // Read-only handle for end-to-end tests and debugging.
    (window as unknown as { __kestrel: unknown }).__kestrel = { getState: useShip.getState, live: shipLive };
  }, []);
  return (
    <div className="app">
      <SceneErrorBoundary>
        <Stage />
      </SceneErrorBoundary>
      <Console />
    </div>
  );
}
