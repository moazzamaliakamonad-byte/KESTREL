"use client";

import { useRef } from "react";
import { useAnimationFrame } from "@/hooks/useAnimationFrame";
import { solveStats } from "@/lib/flightRuntime";
import { fmtMass, fmtNumber } from "@/lib/format";
import { live } from "@/lib/live";
import { glideMargin, thrustShare, tiltMargin } from "@/lib/margins";
import { useApp } from "@/lib/store";
import s from "./hud.module.css";

type Field = "glide" | "tilt" | "thrust" | "prop" | "solve" | "rate";
const W = 220;
const H = 34;
/** Window for the re-plan rate, ms of real time. */
const RATE_WINDOW = 2000;

const MODE_LABEL = { none: "Planning", pinpoint: "Pinpoint", closest: "Closest", brake: "Braking" } as const;

/** Live constraint margins and guidance timing during a flight. */
export function Margins() {
  const phase = useApp((st) => st.phase);
  const mode = useApp((st) => st.flight?.mode ?? "none");
  const refs = useRef<Partial<Record<Field, HTMLElement | null>>>({});
  const bands = useRef<{ thrust: HTMLDivElement | null; band: HTMLDivElement | null }>({ thrust: null, band: null });
  const chart = useRef<HTMLCanvasElement>(null);
  const drawn = useRef(-1);
  const history = useRef<{ at: number; replans: number }[]>([]);

  useAnimationFrame((now) => {
    const snap = live.snapshot;
    const vehicle = useApp.getState().vehicle;
    if (!snap || !vehicle) return;
    const set = (f: Field, text: string, bad = false) => {
      const el = refs.current[f];
      if (!el) return;
      if (el.textContent !== text) el.textContent = text;
      el.dataset.bad = String(bad);
    };
    const glide = glideMargin(snap.position, vehicle.glide_slope);
    set("glide", `${fmtNumber(glide, 1)}°`, glide < 0);
    const tilt = tiltMargin(snap.thrust, vehicle.max_tilt);
    set("tilt", tilt === null ? "engine off" : `${fmtNumber(tilt, 1)}°`, tilt !== null && tilt < 0.5);
    const share = thrustShare(snap.thrust, snap.max_thrust);
    set("thrust", `${Math.round(share * 100)}% of ${fmtNumber(snap.max_thrust / 1000, 1)} kN`, share > 0.995);
    const left = Math.max(snap.mass - vehicle.dry_mass, 0);
    set("prop", fmtMass(left), left < 20);
    if (bands.current.thrust) bands.current.thrust.style.width = `${Math.min(share, 1) * 100}%`;
    if (bands.current.band) {
      const lo = snap.min_thrust / vehicle.max_thrust;
      const hi = snap.max_thrust / vehicle.max_thrust;
      bands.current.band.style.left = `${lo * 100}%`;
      bands.current.band.style.width = `${(hi - lo) * 100}%`;
    }

    const { replans, solveMs } = solveStats();
    const h = history.current;
    h.push({ at: now, replans });
    while (h.length > 2 && now - h[0].at > RATE_WINDOW) h.shift();
    const span = (h[h.length - 1].at - h[0].at) / 1000;
    const rate = span > 0.5 ? (h[h.length - 1].replans - h[0].replans) / span : 0;
    set("rate", `${fmtNumber(rate, 1)} Hz`);
    const last = solveMs.at(-1);
    set("solve", last === undefined ? "—" : `${fmtNumber(last, 0)} ms`);

    if (drawn.current !== live.solveVersion) {
      drawn.current = live.solveVersion;
      drawSolves(chart.current, solveMs);
    }
  });

  if (phase !== "flying" && phase !== "touchdown") return null;

  const row = (label: string, field: Field) => (
    <div className={s.row}>
      <span className={s.label}>{label}</span>
      <span className={`${s.value} ${s.margin}`} ref={(el) => void (refs.current[field] = el)}>
        —
      </span>
    </div>
  );

  return (
    <section className={`${s.margins} ${s.glass}`} aria-label="Constraint margins">
      <div className={s.panelHead}>
        <span className={s.panelTitle}>Guidance</span>
        <span className={`${s.badge} ${s[`badge-${mode}`]}`} data-testid="mode">
          {MODE_LABEL[mode]}
        </span>
      </div>
      {row("Glide-slope margin", "glide")}
      {row("Tilt margin", "tilt")}
      {row("Thrust", "thrust")}
      <div className={s.thrustBar} aria-hidden>
        <div className={s.thrustBand} ref={(el) => void (bands.current.band = el)} />
        <div className={s.thrustFill} ref={(el) => void (bands.current.thrust = el)} />
      </div>
      {row("Propellant left", "prop")}
      {row("Last re-plan", "solve")}
      {row("Re-plan rate", "rate")}
      <canvas ref={chart} className={s.solveChart} style={{ width: W, height: H }} aria-label="Recent solve times" />
    </section>
  );
}

/** Bars of recent solve times, scaled to the slowest (at least 50 ms). */
function drawSolves(c: HTMLCanvasElement | null, ms: readonly number[]) {
  const ctx = c?.getContext("2d");
  if (!c || !ctx) return;
  const dpr = window.devicePixelRatio || 1;
  c.width = W * dpr;
  c.height = H * dpr;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, W, H);
  const top = Math.max(50, ...ms);
  const bw = W / 40;
  ms.forEach((v, i) => {
    const h = Math.max(1, (v / top) * (H - 2));
    ctx.fillStyle = v > 150 ? "#ffb35c" : "#7fe3ff";
    ctx.fillRect(i * bw + 1, H - h, bw - 2, h);
  });
}
