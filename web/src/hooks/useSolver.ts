"use client";

import { useEffect } from "react";
import { createSolverClient, type SolverClient } from "@/lib/solverClient";
import { useApp } from "@/lib/store";

/** Wait this long after the last drag movement before solving. */
const DEBOUNCE_MS = 120;

/**
 * Starts the solver worker, loads the example start state, and re-solves
 * (debounced, newest-wins) whenever the start state changes while editing.
 */
export function useSolver() {
  useEffect(() => {
    const worker = new Worker(new URL("../workers/solver.worker.ts", import.meta.url), {
      type: "module",
    });
    const { dispatch, setSolveMs, setVehicle } = useApp.getState();
    const fail = (message: string) => dispatch({ type: "workerError", message });
    worker.addEventListener("error", (e) => fail(`Solver worker crashed: ${e.message || "unknown error"}`));

    const client: SolverClient = createSolverClient(
      worker,
      (id, response, ms) => {
        dispatch({ type: "response", id, response });
        setSolveMs(ms);
        if (response.ok) setVehicle(response.vehicle);
      },
      fail,
    );

    const solve = () => {
      const { start, vehicle } = useApp.getState();
      const id = client.request({ ...start, vehicle: vehicle ?? undefined });
      dispatch({ type: "requested", id, at: performance.now() });
    };

    let timer: ReturnType<typeof setTimeout> | undefined;
    const unsubscribe = useApp.subscribe((s, prev) => {
      if (s.phase === "editing" && s.start !== prev.start) {
        clearTimeout(timer);
        timer = setTimeout(solve, DEBOUNCE_MS);
      }
    });

    client.loadExample().then((example) => {
      if (example.vehicle) setVehicle(example.vehicle);
      dispatch({ type: "ready", start: { position: example.position, velocity: example.velocity } });
      solve();
    });

    return () => {
      unsubscribe();
      clearTimeout(timer);
      worker.terminate();
    };
  }, []);
}
