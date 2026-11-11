"use client";

import { useApp } from "@/lib/store";
import s from "./hud.module.css";

/** Full-page message when the solver can't run at all. */
export function Fallback() {
  const phase = useApp((st) => st.phase);
  const failure = useApp((st) => st.failure);
  if (phase !== "failed") return null;
  return (
    <div className={s.fallback} role="alert">
      <div className={`${s.fallbackCard} ${s.glass}`}>
        <h2>The guidance solver couldn&apos;t start</h2>
        <p>{failure}</p>
        <button className={s.land} onClick={() => window.location.reload()}>
          Reload
        </button>
      </div>
    </div>
  );
}
