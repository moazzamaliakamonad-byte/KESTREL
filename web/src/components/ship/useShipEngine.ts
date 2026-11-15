"use client";

import { useEffect } from "react";
import { ship_scenario } from "@/wasm/kestrel_wasm";
import { createSolverClient } from "@/lib/solverClient";
import { currentShip, loadShipWasm, setShipClient, shipClient, startShip, stopShip } from "@/lib/ship/runtime";
import { useShip } from "@/lib/ship/store";
import type { Preview, ScenarioInfo, ScenarioKind } from "@/lib/ship/types";

/** Debounce for preview solves after an edit, ms. */
const PREVIEW_DELAY = 250;

/**
 * Loads the simulator (main thread) and starts the guidance worker, reads
 * both scenarios' numbers from Rust, recomputes the preview after edits,
 * and starts/stops flights as the mode changes.
 */
export function useShipEngine() {
  useEffect(() => {
    const st = useShip.getState();
    const fail = (message: string) => useShip.getState().set({ error: message });
    let worker: Worker;
    try {
      worker = new Worker(new URL("../../workers/solver.worker.ts", import.meta.url), { type: "module" });
    } catch (e) {
      fail(`Could not start the guidance worker: ${String(e)}`);
      return;
    }
    worker.addEventListener("error", (e) => fail(`Guidance worker crashed: ${e.message || "unknown error"}`));
    setShipClient(createSolverClient(worker, () => {}, fail));

    let previewTimer: ReturnType<typeof setTimeout> | undefined;
    let previewSeq = 0;
    const preview = () => {
      clearTimeout(previewTimer);
      previewTimer = setTimeout(() => {
        const s = useShip.getState();
        const client = shipClient();
        if (!client || s.mode !== "editing") return;
        const seq = ++previewSeq;
        s.setPreview(s.preview, true);
        void client.call("preview_ship", { scenario: s.scenario, ...s.start }).then(({ response }) => {
          if (seq !== previewSeq) return; // a newer edit superseded it
          useShip.getState().setPreview(response as Preview, false);
        });
      }, PREVIEW_DELAY);
    };

    const loading = loadShipWasm()
      .then(() => {
        for (const k of ["earth", "mars"] as ScenarioKind[]) {
          const info = JSON.parse(ship_scenario(k)) as ScenarioInfo & { ok: boolean };
          if (info.ok) st.setInfo(k, info);
        }
        preview();
      })
      .catch((e: unknown) => fail(`Could not load the simulator: ${String(e)}`));

    const unsubscribe = useShip.subscribe((s, prev) => {
      if (s.mode === "editing" && (s.start !== prev.start || s.scenario !== prev.scenario || prev.mode !== "editing")) {
        preview();
      }
      if (s.mode === "flying" && prev.mode !== "flying") {
        void loading.then(() => {
          const now = useShip.getState();
          if (now.mode !== "flying") return;
          if (!startShip(now.scenario, now.start, now.lag, now.noise)) {
            fail("The simulator refused this start.");
            now.reset();
          }
        });
      }
      if (s.mode === "editing" && prev.mode !== "editing") stopShip();
      const f = currentShip();
      if (f && (s.lag !== prev.lag || s.noise !== prev.noise)) {
        f.set_lag(s.lag);
        f.set_noise(s.noise);
      }
    });

    return () => {
      unsubscribe();
      clearTimeout(previewTimer);
      stopShip();
      setShipClient(null);
      worker.terminate();
    };
  }, []);
}
