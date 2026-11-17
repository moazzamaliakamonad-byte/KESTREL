"use client";

import { shown } from "@/lib/ship/shown";
import { useEffect, useRef } from "react";
import { rotate } from "@/lib/ship/frames";
import { shipLive, type Sample } from "@/lib/ship/live";
import { failEngine } from "@/lib/ship/runtime";
import { PHASE_LABEL, useShip } from "@/lib/ship/store";
import type { ShipSnapshot } from "@/lib/ship/types";
import { EngineMap } from "./Dock";
import { Panel, fmt, useTick } from "./controls";
import { Units } from "./Units";
import css from "./console.module.css";

/** Where the nose points, seen from above: centre = upright, rim = horizontal. */
function AttitudeBall({ s }: { s: ShipSnapshot }) {
  const nose = rotate(s.quat, [0, 0, 1]);
  const tilt = (s.tilt_deg / 90) * 42;
  const h = Math.hypot(nose[0], nose[1]) || 1;
  const [x, y] = [50 + (nose[0] / h) * tilt, 50 - (nose[1] / h) * tilt];
  const speed = Math.hypot(s.velocity[0], s.velocity[1]) || 1;
  return (
    <svg className={css.ball} viewBox="0 0 100 100" aria-label={`Tilt ${fmt(s.tilt_deg, 1)} degrees`}>
      <circle cx={50} cy={50} r={42} className={css.ballRim} />
      <circle cx={50} cy={50} r={21} className={css.ballRing} />
      <circle cx={50} cy={50} r={7} className={css.ballRing} />
      <line x1={8} x2={92} y1={50} y2={50} className={css.ballRing} />
      <line x1={50} x2={50} y1={8} y2={92} className={css.ballRing} />
      {/* Ground track: which way it is drifting. */}
      {Math.hypot(s.velocity[0], s.velocity[1]) > 0.5 && (
        <line x1={50} y1={50} x2={50 + (s.velocity[0] / speed) * 40} y2={50 - (s.velocity[1] / speed) * 40} className={css.ballTrack} />
      )}
      <line x1={50} y1={50} x2={x} y2={y} className={css.ballNeedle} />
      <circle cx={x} cy={y} r={4} className={css.ballDot} />
      <text x={50} y={99} className={css.ballText}>
        −x tower ◄ ► +x
      </text>
    </svg>
  );
}

function Bar({ label, value, text, marks, tone }: { label: string; value: number; text: string; marks?: number[]; tone?: "warm" | "ok" }) {
  return (
    <div className={css.bar}>
      <span>
        {label}
        <b>{text}</b>
      </span>
      <div data-tone={tone}>
        <i style={{ width: `${Math.max(0, Math.min(1, value)) * 100}%` }} />
        {marks?.map((m, i) => <u key={i} style={{ left: `${m * 100}%` }} />)}
      </div>
    </div>
  );
}

/** Solve times as a tiny line. */
export function Spark({ values }: { values: number[] }) {
  if (values.length < 2) return <svg className={css.spark} viewBox="0 0 100 20" />;
  const max = Math.max(...values, 1);
  const d = values.map((v, i) => `${i ? "L" : "M"}${((i / (values.length - 1)) * 100).toFixed(1)},${(19 - (v / max) * 17).toFixed(1)}`).join("");
  return (
    <svg className={css.spark} viewBox="0 0 100 20" preserveAspectRatio="none" aria-hidden>
      <path d={d} />
    </svg>
  );
}

/** One quantity's history, drawn on a canvas. */
export function Graph({ pick, label, unit, scale = 1, digits = 0 }: { pick: (s: Sample) => number; label: string; unit: string; scale?: number; digits?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const value = useRef<HTMLElement>(null);
  useEffect(() => {
    const draw = () => {
      const c = ref.current;
      const h = shipLive.history;
      if (!c) return;
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      const [w, hh] = [c.clientWidth, c.clientHeight];
      if (c.width !== w * dpr || c.height !== hh * dpr) {
        c.width = w * dpr;
        c.height = hh * dpr;
      }
      const g = c.getContext("2d");
      if (!g) return;
      g.setTransform(dpr, 0, 0, dpr, 0, 0);
      g.clearRect(0, 0, w, hh);
      g.strokeStyle = "rgba(140, 220, 255, 0.14)";
      g.lineWidth = 1;
      for (let i = 1; i < 4; i++) {
        g.beginPath();
        g.moveTo(0, (hh * i) / 4);
        g.lineTo(w, (hh * i) / 4);
        g.stroke();
      }
      if (h.length < 2) return;
      const ys = h.map((s) => pick(s) * scale);
      const [t0, t1] = [h[0].t, h[h.length - 1].t];
      const [lo, hi] = [Math.min(0, ...ys), Math.max(...ys, 1e-9)];
      const X = (t: number) => ((t - t0) / Math.max(1e-6, t1 - t0)) * (w - 2) + 1;
      const Y = (y: number) => hh - 2 - ((y - lo) / (hi - lo)) * (hh - 6);
      g.beginPath();
      h.forEach((s, i) => (i ? g.lineTo(X(s.t), Y(ys[i])) : g.moveTo(X(s.t), Y(ys[i]))));
      g.strokeStyle = "#9fe9ff";
      g.lineWidth = 1.6;
      g.shadowColor = "#5ad1ff";
      g.shadowBlur = 6;
      g.stroke();
      g.shadowBlur = 0;
      g.lineTo(X(t1), hh);
      g.lineTo(X(t0), hh);
      g.closePath();
      g.fillStyle = "rgba(90, 209, 255, 0.12)";
      g.fill();
      if (value.current) value.current.textContent = `${ys[ys.length - 1].toFixed(digits)} ${unit}`;
      if (value.current) value.current.dataset.unit = unit;
    };
    draw();
    const id = setInterval(draw, 200);
    return () => clearInterval(id);
  }, [pick, scale, unit, digits]);
  return (
    <figure className={css.graph}>
      <figcaption>
        {label}
        <b ref={value} />
      </figcaption>
      <canvas ref={ref} />
    </figure>
  );
}

function Readout({ label, value, unit, sub }: { label: string; value: string; unit: string; sub?: string }) {
  return (
    <div className={css.readout}>
      <span>{label}</span>
      <b>
        {value}
        <i className={css.unit} data-unit={unit}>
          {unit}
        </i>
      </b>
      {sub && (
        <em>
          <Units text={sub} />
        </em>
      )}
    </div>
  );
}

/** Live numbers, refreshed about 10 times a second from `shipLive`. */
export function Telemetry() {
  useTick(100);
  const vehicle = useShip((s) => s.vehicle);
  const flight = useShip((s) => s.flight);
  const info = useShip((s) => s.info[s.scenario]);
  const s = shown();
  if (!s || !info) return null;
  const speed = Math.hypot(...s.velocity);
  const z = s.position[2];
  const full = info.propellant;
  const lit = s.engines.filter((e) => e === "running").length;
  const max = info.ship.engine_max * info.ship.engines;
  const solve = shipLive.solveMs.at(-1);
  const guide =
    flight?.guide === "plan" ? `SCvx plan · ${flight.legEngines} engine${flight.legEngines === 1 ? "" : "s"}` : flight?.guide === "brake" ? "No plan · braking" : s.phase === "landing-burn" ? "Waiting for a plan" : lit ? "Scripted control" : "Unpowered steering";
  return (
    <Panel className={css.telemetry}>
      <header className={css.phase}>
        <span>{vehicle === "booster" ? "Booster" : "Ship"}</span>
        <h2 data-testid="phase">{PHASE_LABEL[s.phase]}</h2>
      </header>
      <div className={css.big}>
        <Readout label="Altitude" value={z >= 10_000 ? fmt(z / 1000, 1) : z >= 1000 ? fmt(z / 1000, 2) : fmt(z)} unit={z >= 1000 ? "km" : "m"} />
        <Readout label="Speed" value={fmt(speed)} unit="m/s" sub={`${fmt(speed * 3.6)} km/h · Mach ${fmt(s.mach, 2)}`} />
      </div>
      <div className={css.grid}>
        <span>Descent rate</span>
        <b>
          <Units text={`${fmt(-s.velocity[2], 1)} m/s`} />
        </b>
        <span>Tilt</span>
        <b>{fmt(s.tilt_deg, 1)}°</b>
        <span>Dynamic pressure</span>
        <b>
          <Units text={`${fmt(s.q_dyn / 1000, 1)} kPa`} />
        </b>
        <span>Thrust / weight</span>
        <b>{fmt(s.thrust_mag / Math.max(1, s.weight), 2)}</b>
        <span>Heating</span>
        <b>
          <Units text={`${fmt(s.heat_flux / 1000, s.heat_flux < 1e4 ? 1 : 0)} kW/m²`} />
        </b>
        <span>Mass</span>
        <b>
          <Units text={`${fmt(s.mass / 1000, 1)} t`} />
        </b>
      </div>
      <Bar label="Propellant" value={s.propellant / full} text={`${fmt(s.propellant / 1000, 1)} t`} tone={s.propellant / full < 0.15 ? "warm" : undefined} />
      <Bar label={`Thrust · ${lit} lit`} value={s.thrust_mag / max} text={`${fmt(s.thrust_mag / 1e6, 2)} MN`} marks={lit ? [s.thrust_limits[0] / max, s.thrust_limits[1] / max] : []} tone="warm" />
      <div className={css.instruments}>
        <AttitudeBall s={s} />
        <div>
          <EngineMap vehicle={vehicle} states={s.engines} failed={[]} onToggle={(i) => failEngine(i)} />
          <p className={css.hint}>Click an engine to fail it</p>
        </div>
      </div>
      <div className={css.surfaces} aria-label={vehicle === "ship" ? "Flaps" : "Grid fins"}>
        <span>{vehicle === "ship" ? "Flaps" : "Grid fins"}</span>
        {(vehicle === "ship" ? s.flaps : s.flaps.slice(0, 3)).map((f, i) =>
          vehicle === "ship" ? (
            <i key={i} title={["fore left", "fore right", "aft left", "aft right"][i]}>
              <u style={{ height: `${Math.round(f * 100)}%` }} />
            </i>
          ) : (
            <i key={i} data-fin title={["+y fin", "−y fin", "+x fin"][i]}>
              <u style={{ transform: `rotate(${fmt((f * 180) / Math.PI, 1)}deg)` }} />
            </i>
          ),
        )}
        <em data-on={s.rcs.some(Boolean) || undefined}>RCS</em>
        <em data-on={s.arms || undefined}>Arms</em>
      </div>
      <div className={css.guidance}>
        <span>Guidance</span>
        <b>{guide}</b>
        <span>Last solve</span>
        <b>{solve !== undefined ? `${fmt(solve)} ms · ${shipLive.replans} plans` : "—"}</b>
        {s.engine_out_margin !== null && s.engine_out_margin !== undefined && (
          <>
            <span title="If the running engine failed now, could a spare (lit 1 s later) still stop the ship above the arms? A point-mass estimate.">Engine-out</span>
            <b data-tone={s.engine_out_margin < 0 ? "warm" : undefined}>
              {s.engine_out_margin >= 0 ? (
                <Units text={`survivable · ${fmt(s.engine_out_margin, 1)} m/s spare`} />
              ) : (
                <Units text={`not survivable · ${fmt(-s.engine_out_margin, 1)} m/s too fast`} />
              )}
            </b>
          </>
        )}

        <Spark values={shipLive.solveMs} />
      </div>
    </Panel>
  );
}

/** Before a flight: what the vehicle is, and which numbers are published. */
export function Briefing() {
  const info = useShip((s) => s.info[s.scenario]);
  const vehicle = useShip((s) => s.vehicle);
  if (!info) return null;
  const rows: [string, string, "pub" | "ill", string][] =
    vehicle === "booster"
      ? [
          ["Height × diameter", `${info.ship.length} m × ${info.ship.radius * 2} m`, "pub", "size"],
          ["Engines", "33 · inner 13 gimbal", "pub", "engines"],
          ["Thrust per engine", `${fmt(info.ship.engine_max / 1e6, 2)} MN`, "pub", "thrust"],
          ["Specific impulse", `${info.ship.isp} s (sea level)`, "pub", "isp"],
          ["Throttle range", `${fmt(info.ship.throttle_min * 100)}–100 %`, "pub", "throttle"],
          ["Grid fins", "3 · 90/90/180°", "pub", "engines"],
          ["Empty mass", `${fmt(info.dry_mass / 1000)} t`, "ill", "dryMass"],
          ["Landing propellant", `${fmt(info.propellant / 1000)} t`, "ill", "propellant"],
          ["Gimbal limit", `±${fmt(info.ship.gimbal_max_deg)}°`, "ill", "gimbal"],
          ["Aerodynamics, inertia", "simplified model", "ill", "aero"],
        ]
      : [
          ["Height × diameter", `${info.ship.length} m × ${info.ship.radius * 2} m`, "pub", "size"],
          ["Engines", "3 sea-level + 3 vacuum", "pub", "engines"],
          ["Thrust per engine", `${fmt(info.ship.engine_max / 1e6, 2)} MN`, "pub", "thrust"],
          ["Specific impulse", `${info.ship.isp} s (sea level)`, "pub", "isp"],
          ["Flaps", "4 · two forward, two aft", "pub", "engines"],
          ["Empty mass", `${fmt(info.dry_mass / 1000)} t`, "ill", "dryMass"],
          ["Landing propellant", `${fmt(info.propellant / 1000)} t`, "ill", "propellant"],
          ["Gimbal limit", `±${fmt(info.ship.gimbal_max_deg)}°`, "ill", "gimbal"],
          ["Aerodynamics, inertia", "simplified model", "ill", "aero"],
        ];
  return (
    <Panel title={vehicle === "booster" ? "Super Heavy-class booster" : "Starship-class ship"} className={css.briefing}>
      <dl className={css.facts}>
        {rows.map(([k, v, tag, src]) => (
          <div key={k}>
            <dt>{k}</dt>
            <dd>
              <Units text={v} />
              <i data-tag={tag} data-note={src}>
                {tag === "pub" ? "published" : "illustrative"}
              </i>
            </dd>
          </div>
        ))}
      </dl>
      <p className={css.hint}>
        A simulation. Published figures are from public sources; illustrative ones have not been published and were chosen to be plausible. Not flight software, not any company&apos;s data.
      </p>
    </Panel>
  );
}
