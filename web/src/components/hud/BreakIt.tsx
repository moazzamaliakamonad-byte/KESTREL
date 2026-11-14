"use client";

import { currentFlight } from "@/lib/flightRuntime";
import type { NoiseLevel } from "@/lib/state";
import { useApp } from "@/lib/store";
import s from "./hud.module.css";

const LAGS: { value: number; label: string }[] = [
  { value: 0, label: "Off" },
  { value: 0.3, label: "0.3 s" },
  { value: 1, label: "1 s" },
];
const NOISE: { value: NoiseLevel; label: string }[] = [
  { value: "off", label: "Off" },
  { value: "low", label: "Low" },
  { value: "high", label: "High" },
];

/**
 * "Break it": disturbances the visitor throws at the vehicle. Lag and noise
 * can be set before Land; gusts and engine failures act on a flight.
 */
export function BreakIt() {
  const phase = useApp((st) => st.phase);
  const settings = useApp((st) => st.settings);
  const engines = useApp((st) => st.flight?.engines ?? 6);
  const { dispatch } = useApp.getState();
  const flying = phase === "flying";
  if (phase !== "editing" && !flying) return null;

  const gust = (dir: number) => currentFlight()?.gust(dir);
  const engineOut = () => {
    // The store learns the new count from the next snapshot.
    currentFlight()?.engine_out();
  };

  return (
    <section className={`${s.breakit} ${s.glass}`} aria-label="Break it">
      <div className={s.panelTitle}>Break it</div>
      <div className={s.breakRow}>
        <button className={s.hit} disabled={!flying} onClick={() => gust(-1)} title="Sideways push, 2 s">
          Gust ←
        </button>
        <button className={s.hit} disabled={!flying} onClick={() => gust(1)} title="Sideways push, 2 s">
          Gust →
        </button>
      </div>
      <button
        className={`${s.hit} ${s.danger}`}
        disabled={!flying || engines <= 3}
        onClick={engineOut}
        title="Fail one of the six engines"
      >
        Engine out <span className={s.engines}>{engines}/6</span>
      </button>
      <div className={s.label}>Thrust lag</div>
      <div className={s.group} role="group" aria-label="Thrust lag">
        {LAGS.map((l) => (
          <button
            key={l.label}
            className={s.seg}
            aria-pressed={settings.lag === l.value}
            onClick={() => dispatch({ type: "setLag", lag: l.value })}
          >
            {l.label}
          </button>
        ))}
      </div>
      <div className={s.label}>Sensor noise</div>
      <div className={s.group} role="group" aria-label="Sensor noise">
        {NOISE.map((n) => (
          <button
            key={n.value}
            className={s.seg}
            aria-pressed={settings.noise === n.value}
            onClick={() => dispatch({ type: "setNoise", noise: n.value })}
          >
            {n.label}
          </button>
        ))}
      </div>
      {!flying && <p className={s.panelHint}>Gusts and engine failures work once you press Land. You can also grab the booster mid-flight.</p>}
    </section>
  );
}
