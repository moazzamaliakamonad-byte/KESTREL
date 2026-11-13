"use client";

import { useEffect, useRef } from "react";
import { useAnimationFrame } from "@/hooks/useAnimationFrame";
import { live } from "@/lib/live";
import { sampleAt } from "@/lib/playback";
import type { PlanSuccess } from "@/lib/types";

const W = 260;
const H = 64;
const N = 160;

interface Props {
  plan: PlanSuccess | null;
  /** Value plotted at time t. */
  value: (plan: PlanSuccess, t: number) => number;
  /** Horizontal guide lines (e.g. thrust limits), with labels. */
  guides: (plan: PlanSuccess) => { y: number; label: string }[];
  /** [min, max] of the vertical axis. */
  range: (plan: PlanSuccess) => [number, number];
  color: string;
  label: string;
}

/** A small chart of one quantity over the planned landing, with a moving cursor. */
export function Sparkline({ plan, value, guides, range, color, label }: Props) {
  const base = useRef<HTMLCanvasElement>(null);
  const cursor = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const c = base.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = W * dpr;
    c.height = H * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    if (!plan) return;
    const [lo, hi] = range(plan);
    const y = (v: number) => H - 4 - ((v - lo) / (hi - lo)) * (H - 8);
    const tf = plan.trajectory.final_time;

    ctx.font = "9px ui-monospace, monospace";
    for (const g of guides(plan)) {
      ctx.strokeStyle = "rgba(255,255,255,0.22)";
      ctx.setLineDash([3, 3]);
      ctx.beginPath();
      ctx.moveTo(0, y(g.y));
      ctx.lineTo(W, y(g.y));
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillStyle = "rgba(255,255,255,0.45)";
      ctx.fillText(g.label, W - ctx.measureText(g.label).width - 2, y(g.y) - 3);
    }

    ctx.strokeStyle = color;
    ctx.lineWidth = 1.6;
    ctx.shadowColor = color;
    ctx.shadowBlur = 6;
    ctx.beginPath();
    for (let i = 0; i <= N; i++) {
      const t = (tf * i) / N;
      const px = (W * i) / N;
      const py = y(value(plan, Math.min(t, tf * 0.9999)));
      if (i === 0) ctx.moveTo(px, py);
      else ctx.lineTo(px, py);
    }
    ctx.stroke();
    ctx.shadowBlur = 0;
  }, [plan, value, guides, range, color]);

  useAnimationFrame(() => {
    const c = cursor.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    if (c.width !== W * dpr) {
      c.width = W * dpr;
      c.height = H * dpr;
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    if (!plan || !live.sample) return;
    const x = (W * live.sample.t) / plan.trajectory.final_time;
    ctx.fillStyle = "rgba(255,255,255,0.85)";
    ctx.fillRect(x - 0.75, 0, 1.5, H);
  });

  return (
    <div>
      <div style={{ color: "var(--muted)", fontSize: 10, letterSpacing: "0.1em", textTransform: "uppercase", marginBottom: 4 }}>
        {label}
      </div>
      <div style={{ position: "relative", width: W, height: H }}>
        <canvas ref={base} style={{ width: W, height: H, position: "absolute", inset: 0 }} />
        <canvas ref={cursor} style={{ width: W, height: H, position: "absolute", inset: 0 }} />
      </div>
    </div>
  );
}

/** Thrust magnitude with the engine's min/max band. */
export const thrustChart = {
  value: (p: PlanSuccess, t: number) => sampleAt(p.trajectory, p.vehicle, t).thrustMag,
  guides: (p: PlanSuccess) => [
    { y: p.vehicle.max_thrust, label: "max" },
    { y: p.vehicle.min_thrust, label: "min" },
  ],
  range: (p: PlanSuccess): [number, number] => [0, p.vehicle.max_thrust * 1.1],
};

/** Vehicle mass with the dry-mass floor. */
export const massChart = {
  value: (p: PlanSuccess, t: number) => sampleAt(p.trajectory, p.vehicle, t).mass,
  guides: (p: PlanSuccess) => [{ y: p.vehicle.dry_mass, label: "dry" }],
  range: (p: PlanSuccess): [number, number] => [p.vehicle.dry_mass * 0.98, p.vehicle.wet_mass * 1.005],
};
