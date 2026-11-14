"use client";

import { useApp } from "@/lib/store";
import s from "./hud.module.css";

/** A banner that says so, loudly, when guidance gives up on the pad. */
export function ModeBanner() {
  const phase = useApp((st) => st.phase);
  const mode = useApp((st) => st.flight?.mode);
  const miss = useApp((st) => st.flight?.miss ?? 0);
  if (phase !== "flying") return null;
  if (mode === "closest" && miss >= 1) {
    return (
      <div className={`${s.banner} ${s["banner-closest"]} ${s.glass}`} role="alert">
        No pinpoint landing exists from here. Guiding to the closest reachable point, about{" "}
        <b>{Math.round(miss)} m</b> from the pad.
      </div>
    );
  }
  if (mode === "brake") {
    return (
      <div className={`${s.banner} ${s["banner-brake"]} ${s.glass}`} role="alert">
        No safe landing exists. Full thrust upward to hit the ground as slowly as possible.
      </div>
    );
  }
  return null;
}
