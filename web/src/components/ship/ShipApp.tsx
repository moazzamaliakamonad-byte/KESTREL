"use client";

import { SceneErrorBoundary } from "@/components/hud/Fallback";
import { shipLive } from "@/lib/ship/live";
import { useShip } from "@/lib/ship/store";
import { useEffect } from "react";
import { ShipHud } from "./ShipHud";
import { ShipScene } from "./ShipScene";
import { useShipEngine } from "./useShipEngine";

/** The M3 page: a Starship-class ship's return, Earth (tower catch) or Mars (legs). */
export default function ShipApp() {
  useShipEngine();
  useEffect(() => {
    // Read-only handle for end-to-end tests and debugging.
    (window as unknown as { __kestrelShip: unknown }).__kestrelShip = { getState: useShip.getState, live: shipLive };
  }, []);
  return (
    <div className="app">
      <SceneErrorBoundary>
        <ShipScene />
      </SceneErrorBoundary>
      <ShipHud />
    </div>
  );
}
