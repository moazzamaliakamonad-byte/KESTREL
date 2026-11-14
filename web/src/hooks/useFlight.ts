"use client";

import { useEffect } from "react";
import { currentFlight, loadFlightWasm, startFlight, stopFlight } from "@/lib/flightRuntime";
import { useApp } from "@/lib/store";

/**
 * Starts a closed-loop flight when the visitor presses Land, ends it on
 * Reset, and passes lag/noise changes to the running flight.
 */
export function useFlight() {
  useEffect(() => {
    const { dispatch } = useApp.getState();
    const loading = loadFlightWasm().catch((e: unknown) =>
      dispatch({ type: "workerError", message: `Could not load the simulator: ${String(e)}` }),
    );

    const unsubscribe = useApp.subscribe((s, prev) => {
      if (s.phase === "flying" && prev.phase !== "flying") {
        const { start, vehicle, settings } = s;
        if (!vehicle) return;
        void loading.then(() => {
          // Pressed Reset before the module finished loading: do nothing.
          if (useApp.getState().phase !== "flying") return;
          const ok = startFlight({
            position: start.position,
            velocity: start.velocity,
            vehicle,
            seed: Math.floor(Math.random() * 2 ** 32),
            lag: settings.lag,
            noise: settings.noise,
          });
          if (!ok) dispatch({ type: "workerError", message: "The simulator rejected this start state." });
        });
      }
      if (s.phase === "editing" && prev.phase !== "editing") stopFlight();
      const f = currentFlight();
      if (f && s.settings !== prev.settings) {
        f.set_lag(s.settings.lag);
        f.set_noise(s.settings.noise);
      }
    });
    return () => {
      unsubscribe();
      stopFlight();
    };
  }, []);
}
