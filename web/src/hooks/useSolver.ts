"use client";

import { useEffect } from "react";
import { createScheduler, startChangedWhileEditing } from "@/lib/scheduler";
import { setSolverClient } from "@/lib/flightRuntime";
import { createSolverClient } from "@/lib/solverClient";
import { useApp } from "@/lib/store";

/**
 * Starts the solver worker, loads the example start state, and re-solves
 * whenever the start changes while editing: at most one solve in flight,
 * newest start next (see `createScheduler`).
 */
export function useSolver() {
  useEffect(() => {
    const { dispatch, setSolveMs, setVehicle } = useApp.getState();
    const fail = (message: string) => dispatch({ type: "workerError", message });

    let worker: Worker;
    try {
      worker = new Worker(new URL("../workers/solver.worker.ts", import.meta.url), {
        type: "module",
      });
    } catch (e) {
      fail(`Could not start the solver worker: ${String(e)}`);
      return;
    }
    worker.addEventListener("error", (e) => fail(`Solver worker crashed: ${e.message || "unknown error"}`));

    const client = createSolverClient(
      worker,
      (id, response, ms) => {
        dispatch({ type: "response", id, response });
        setSolveMs(ms);
        if (response.ok) setVehicle(response.vehicle);
        scheduler.done();
      },
      fail,
    );

    setSolverClient(client);

    const scheduler = createScheduler((start) => {
      const vehicle = useApp.getState().vehicle;
      const id = client.request({ ...start, vehicle: vehicle ?? undefined });
      dispatch({ type: "requested", id, at: performance.now(), start });
    });

    const unsubscribe = useApp.subscribe((s, prev) => {
      if (startChangedWhileEditing(s, prev)) scheduler.submit(s.start);
    });

    client.loadExample().then((example) => {
      if (example.vehicle) setVehicle(example.vehicle);
      dispatch({ type: "ready", start: { position: example.position, velocity: example.velocity } });
      scheduler.submit(useApp.getState().start);
    });

    return () => {
      unsubscribe();
      setSolverClient(null);
      worker.terminate();
    };
  }, []);
}
