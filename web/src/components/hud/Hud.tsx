"use client";

import { useRef, useState } from "react";
import { useAnimationFrame } from "@/hooks/useAnimationFrame";
import { fmtMass, fmtNumber, fmtSpeed } from "@/lib/format";
import { statusText } from "@/lib/status";
import { useApp } from "@/lib/store";
import type { PlanSuccess } from "@/lib/types";
import { Controls } from "./Controls";
import { massChart, Sparkline, thrustChart } from "./Sparkline";
import { Telemetry } from "./Telemetry";
import s from "./hud.module.css";

function Brand() {
  return (
    <header className={s.brand}>
      <div className={s.wordmark}>KESTREL</div>
      <p className={s.pitch}>
        Real fuel-optimal rocket landing guidance, solved live in your browser.
      </p>
      <span className={s.chip}>Simulation only</span>
    </header>
  );
}

function Status() {
  const state = useApp();
  const solveMs = useApp((st) => st.lastSolveMs);
  const [now, setNow] = useState(0);
  // Re-evaluate time-dependent text ("Still planning…") a few times a second.
  const last = useRef(0);
  useAnimationFrame((t) => {
    if (t - last.current > 250) {
      last.current = t;
      setNow(t);
    }
  });
  const { tone, text } = statusText(state, now);
  return (
    <div className={`${s.status} ${s.glass} ${s[`tone-${tone}`]}`} role="status" aria-live="polite">
      <span className={s.dot} />
      <span data-testid="status">{text}</span>
      {solveMs !== null && <span className={s.solveTime}>solve {fmtNumber(solveMs, 0)} ms</span>}
    </div>
  );
}

function Help() {
  const phase = useApp((st) => st.phase);
  if (phase !== "editing") return null;
  return (
    <p className={s.help}>
      Drag the <b>ring</b> to move the booster · drag the <b>orange dot</b> to set its velocity ·{" "}
      <b>Space</b> to land
    </p>
  );
}

function TouchdownCard() {
  const phase = useApp((st) => st.phase);
  const flight = useApp((st) => st.flight);
  const solveMs = useApp((st) => st.lastSolveMs);
  if (phase !== "touchdown" || !flight) return null;
  const tr = flight.response.trajectory;
  const n = tr.velocity.length - 1;
  const [vx, vy, vz] = tr.velocity[n];
  return (
    <section className={`${s.card} ${s.glass}`} aria-label="Touchdown summary">
      <div className={s.cardTitle}>Touchdown</div>
      <div className={s.cardGrid}>
        <div>
          <div className={s.label}>Touchdown speed</div>
          <div className={s.big}>{fmtSpeed(Math.hypot(vx, vy, vz))}</div>
        </div>
        <div>
          <div className={s.label}>Propellant used</div>
          <div className={s.big}>{fmtMass(tr.fuel_used)}</div>
        </div>
        <div>
          <div className={s.label}>Landing time</div>
          <div className={s.big}>{fmtNumber(tr.final_time, 1)} s</div>
        </div>
        <div>
          <div className={s.label}>Solve time</div>
          <div className={s.big}>{solveMs === null ? "—" : `${fmtNumber(solveMs, 0)} ms`}</div>
        </div>
      </div>
      <p className={s.cardHint}>Press Space or Reset to try another start.</p>
    </section>
  );
}

function Charts() {
  const phase = useApp((st) => st.phase);
  const preview = useApp((st) => st.preview);
  const flight = useApp((st) => st.flight);
  const plan: PlanSuccess | null =
    phase === "editing"
      ? preview?.response.ok
        ? preview.response
        : null
      : (flight?.response ?? null);
  return (
    <section className={`${s.charts} ${s.glass}`} aria-label="Charts">
      <Sparkline plan={plan} label="Thrust" color="#ffb35c" {...thrustChart} />
      <Sparkline plan={plan} label="Mass" color="#7fe3ff" {...massChart} />
    </section>
  );
}

/** Everything drawn over the 3D scene. */
export function Hud() {
  return (
    <div className={s.hud}>
      <Brand />
      <Status />
      <Telemetry />
      <Charts />
      <Help />
      <Controls />
      <TouchdownCard />
    </div>
  );
}
