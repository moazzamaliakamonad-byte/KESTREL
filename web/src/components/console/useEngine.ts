"use client";

import { useEffect } from "react";
import { ship_scenario } from "@/wasm/kestrel_wasm";
import { createSolverClient } from "@/lib/solverClient";
import { commentator, currentShip, loadShipWasm, setShipClient, startShip, stopShip } from "@/lib/ship/runtime";
import { configOf, useShip } from "@/lib/ship/store";
import type { Preview, ScenarioInfo, ScenarioKind } from "@/lib/ship/types";

/** Debounce for the prediction after an edit, ms. */
const PREVIEW_DELAY = 300;

/**
 * Loads the simulator (main thread) and starts the guidance worker, reads
 * every flight kind's numbers from Rust, recomputes the prediction after
 * edits, starts and stops flights as the mode changes, and passes live
 * option changes (wind, lag, noise) to the flight in progress.
 */
export function useEngine() {
  useEffect(() => {
    const fail = (message: string) => useShip.getState().setError(message);
    // Two workers: a prediction flies a whole flight (seconds of solving)
    // and must never hold up the plans a flight in progress is waiting for.
    let worker: Worker;
    let previewer: Worker;
    try {
      worker = new Worker(new URL("../../workers/solver.worker.ts", import.meta.url), { type: "module" });
      previewer = new Worker(new URL("../../workers/solver.worker.ts", import.meta.url), { type: "module" });
    } catch (e) {
      fail(`Could not start the guidance worker: ${String(e)}`);
      return;
    }
    for (const w of [worker, previewer]) {
      w.addEventListener("error", (e) => fail(`Guidance worker crashed: ${e.message || "unknown error"}`));
    }
    setShipClient(createSolverClient(worker, () => {}, fail));
    // Fetch the list of commentary clips now, so the first line already has its clip.
    commentator.load();
    const previewClient = createSolverClient(previewer, () => {}, fail);

    let previewTimer: ReturnType<typeof setTimeout> | undefined;
    let previewSeq = 0;
    const preview = () => {
      clearTimeout(previewTimer);
      previewTimer = setTimeout(() => {
        const s = useShip.getState();
        if (s.mode !== "editing") return;
        const seq = ++previewSeq;
        s.setPreview(s.preview, true);
        void previewClient.call("preview_ship", configOf(s)).then(({ response }) => {
          if (seq !== previewSeq) return; // a newer edit superseded it
          useShip.getState().setPreview(response as Preview, false);
        });
      }, PREVIEW_DELAY);
    };

    const loading = loadShipWasm()
      .then(() => {
        for (const k of ["earth", "booster"] as ScenarioKind[]) {
          const info = JSON.parse(ship_scenario(k)) as ScenarioInfo & { ok: boolean };
          if (info.ok) useShip.getState().setInfo(k, info);
        }
        preview();
      })
      .catch((e: unknown) => fail(`Could not load the simulator: ${String(e)}`));

    const unsubscribe = useShip.subscribe((s, prev) => {
      const edited = s.start !== prev.start || s.scenario !== prev.scenario || s.options !== prev.options;
      if (s.mode === "editing" && (edited || prev.mode !== "editing")) preview();
      if (s.mode === "flying" && prev.mode !== "flying") {
        void loading.then(() => {
          const now = useShip.getState();
          if (now.mode !== "flying") return;
          if (!startShip()) {
            fail("The simulator refused this start.");
            now.reset();
          }
        });
      }
      if (s.mode === "editing" && prev.mode !== "editing") stopShip();
      const f = currentShip();
      if (f && s.options !== prev.options) {
        const o = s.options;
        const a = (o.windDir * Math.PI) / 180;
        f.set_lag(o.lag);
        f.set_noise(o.noise);
        f.set_wind(o.windSpeed * Math.cos(a), o.windSpeed * Math.sin(a));
        f.set_gust_speed(o.gustSpeed);
      }
    });

    return () => {
      unsubscribe();
      clearTimeout(previewTimer);
      stopShip();
      setShipClient(null);
      worker.terminate();
      previewer.terminate();
    };
  }, []);
}
