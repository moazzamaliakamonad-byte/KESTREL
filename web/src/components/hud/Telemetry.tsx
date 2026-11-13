"use client";

import { useRef } from "react";
import { useAnimationFrame } from "@/hooks/useAnimationFrame";
import { fmtAltitude, fmtForce, fmtMass, fmtSpeed, fmtTime } from "@/lib/format";
import { live } from "@/lib/live";
import { useApp } from "@/lib/store";
import s from "./hud.module.css";

type Field = "clock" | "alt" | "speed" | "vz" | "thrust" | "prop";

/** Live numbers, written straight to the DOM each frame (no React re-render). */
export function Telemetry() {
  const refs = useRef<Partial<Record<Field, HTMLElement | null>>>({});
  const bar = useRef<HTMLDivElement>(null);

  useAnimationFrame(() => {
    const sample = live.sample;
    const vehicle = useApp.getState().vehicle;
    if (!sample || !vehicle) return;
    const [vx, vy, vz] = sample.velocity;
    const fuel = Math.max(sample.mass - vehicle.dry_mass, 0);
    const capacity = vehicle.wet_mass - vehicle.dry_mass;
    const pct = Math.round((100 * sample.thrustMag) / vehicle.max_thrust);
    const set = (f: Field, text: string) => {
      const el = refs.current[f];
      if (el && el.textContent !== text) el.textContent = text;
    };
    set("clock", fmtTime(sample.t));
    set("alt", fmtAltitude(sample.position[2]));
    set("speed", fmtSpeed(Math.hypot(vx, vy, vz)));
    set("vz", fmtSpeed(vz));
    set("thrust", sample.thrustMag > 0 ? `${fmtForce(sample.thrustMag)} · ${pct}%` : "off");
    set("prop", fmtMass(fuel));
    if (bar.current) bar.current.style.width = `${(100 * fuel) / capacity}%`;
  });

  const row = (label: string, field: Field) => (
    <div className={s.row}>
      <span className={s.label}>{label}</span>
      <span className={s.value} ref={(el) => void (refs.current[field] = el)}>
        —
      </span>
    </div>
  );

  return (
    <section className={`${s.telemetry} ${s.glass}`} aria-label="Telemetry">
      <div className={s.clock} ref={(el) => void (refs.current.clock = el)}>
        T+0.0 s
      </div>
      {row("Altitude", "alt")}
      {row("Speed", "speed")}
      {row("Vertical", "vz")}
      {row("Thrust", "thrust")}
      {row("Propellant", "prop")}
      <div className={s.bar}>
        <div className={s.barFill} ref={bar} />
      </div>
    </section>
  );
}
