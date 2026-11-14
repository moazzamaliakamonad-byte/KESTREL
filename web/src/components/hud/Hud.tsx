"use client";

import { useMemo, useRef, useState } from "react";
import { useAnimationFrame } from "@/hooks/useAnimationFrame";
import { fmtMass, fmtNumber, fmtSpeed } from "@/lib/format";
import { solveStats } from "@/lib/flightRuntime";
import { statusText } from "@/lib/status";
import { useApp } from "@/lib/store";
import type { PlanSuccess } from "@/lib/types";
import { BreakIt } from "./BreakIt";
import { Controls } from "./Controls";
import { Margins } from "./Margins";
import { ModeBanner } from "./ModeBanner";
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
      {solveMs !== null && state.phase === "editing" && (
        <span className={s.solveTime}>solve {fmtNumber(solveMs, 0)} ms</span>
      )}
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

function OutcomeCard() {
  const phase = useApp((st) => st.phase);
  const outcome = useApp((st) => st.flight?.outcome ?? null);
  if (phase !== "touchdown" || !outcome) return null;
  const { replans, allMs } = solveStats();
  const sorted = [...allMs].sort((a, b) => a - b);
  const median = sorted.length ? sorted[Math.floor(sorted.length / 2)] : null;
  return (
    <section className={`${s.card} ${s.glass}`} aria-label="Touchdown summary">
      <div className={`${s.cardTitle} ${outcome.landed ? "" : s.crashed}`}>
        {outcome.landed ? "Landed" : "Crashed"}
      </div>
      <div className={s.cardGrid}>
        <div>
          <div className={s.label}>Touchdown speed</div>
          <div className={s.big}>{fmtSpeed(outcome.speed)}</div>
        </div>
        <div>
          <div className={s.label}>From the pad</div>
          <div className={s.big}>{fmtNumber(outcome.miss, outcome.miss < 10 ? 1 : 0)} m</div>
        </div>
        <div>
          <div className={s.label}>Propellant used</div>
          <div className={s.big}>{fmtMass(outcome.fuel_used)}</div>
        </div>
        <div>
          <div className={s.label}>Re-plans · median solve</div>
          <div className={s.big}>
            {replans} · {median === null ? "—" : `${fmtNumber(median, 0)} ms`}
          </div>
        </div>
      </div>
      <p className={s.cardHint}>Press Space or Reset to try another start.</p>
    </section>
  );
}

function Charts() {
  const phase = useApp((st) => st.phase);
  const preview = useApp((st) => st.preview);
  const flightPlan = useApp((st) => st.flight?.plan ?? null);
  const vehicle = useApp((st) => st.vehicle);
  const plan: PlanSuccess | null = useMemo(() => {
    if (phase === "editing") return preview?.response.ok ? preview.response : null;
    return flightPlan && vehicle ? { ok: true, trajectory: flightPlan.trajectory, vehicle } : null;
  }, [phase, preview, flightPlan, vehicle]);
  const t0 = phase === "editing" ? 0 : (flightPlan?.t0 ?? 0);
  return (
    <section className={`${s.charts} ${s.glass}`} aria-label="Charts">
      <Sparkline plan={plan} t0={t0} label="Planned thrust" color="#ffb35c" {...thrustChart} />
      <Sparkline plan={plan} t0={t0} label="Planned mass" color="#7fe3ff" {...massChart} />
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
      <BreakIt />
      <Margins />
      <ModeBanner />
      <Charts />
      <Help />
      <Controls />
      <OutcomeCard />
    </div>
  );
}
