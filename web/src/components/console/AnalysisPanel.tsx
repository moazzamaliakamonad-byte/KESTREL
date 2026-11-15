"use client";

import { shipLive, type Sample } from "@/lib/ship/live";
import { useShip } from "@/lib/ship/store";
import { Graph } from "./Telemetry";
import { Panel, fmt, useTick } from "./controls";
import css from "./console.module.css";

const altitude = (s: Sample) => s.altitude;
const speed = (s: Sample) => s.speed;
const thrust = (s: Sample) => s.thrust;
const tilt = (s: Sample) => s.tilt;
const q = (s: Sample) => s.q;
const heat = (s: Sample) => s.heat;

function Margin({ label, value, limit, unit, digits = 1 }: { label: string; value: number; limit: number; unit: string; digits?: number }) {
  const f = Math.max(0, Math.min(1, value / limit));
  return (
    <div className={css.margin} data-tight={f > 0.9 || undefined}>
      <span>{label}</span>
      <div>
        <i style={{ width: `${f * 100}%` }} />
      </div>
      <b>
        {fmt(value, digits)} / {fmt(limit, digits)} {unit}
      </b>
    </div>
  );
}

/**
 * The analysis view's panel: the equations the simulator integrates, with
 * this instant's numbers in them, how close guidance is to each limit, and
 * the flight's history as graphs.
 */
export function AnalysisPanel() {
  useTick(150);
  const info = useShip((s) => s.info[s.scenario]);
  const s = shipLive.snapshot;
  if (!s || !info) return null;
  const g0 = 9.80665;
  const mdot = s.thrust_mag / (info.tuning.isp * g0);
  const f = [s.thrust_inertial[0] + s.aero_force[0], s.thrust_inertial[1] + s.aero_force[1], s.thrust_inertial[2] + s.aero_force[2] - s.weight];
  const accel = Math.hypot(...f) / s.mass;
  const omega = (Math.hypot(...s.omega) * 180) / Math.PI;
  const lit = s.engines.filter((e) => e === "running").length;
  return (
    <Panel title="Analysis" className={css.analysis} aside={<span className={css.tagline}>live from the simulator</span>}>
      <div className={css.equations}>
        <p>
          <span>mass</span>
          <code>
            ṁ = −‖T‖ / (I<sub>sp</sub> g<sub>0</sub>)
          </code>
          <b>−{fmt(mdot)} kg/s</b>
        </p>
        <p>
          <span>translation</span>
          <code>
            v̇ = (q T q* + F<sub>aero</sub>) / m − g ẑ
          </code>
          <b>
            {fmt(accel, 1)} m/s² · {fmt(accel / g0, 2)} g
          </b>
        </p>
        <p>
          <span>attitude</span>
          <code>q̇ = ½ q ⊗ ω</code>
          <b>{fmt(omega, 1)} °/s</b>
        </p>
        <p>
          <span>rotation</span>
          <code>
            J ω̇ = r<sub>T</sub> × T + τ<sub>aero</sub> − ω × Jω
          </code>
          <b>δ {fmt(s.gimbal_deg, 1)}°</b>
        </p>
        <p>
          <span>guidance</span>
          <code>
            min −m(t<sub>f</sub>) s.t. x<sub>k+1</sub> = A<sub>k</sub>x<sub>k</sub> + B<sub>k</sub>u<sub>k</sub> + …, ‖T‖ ≤ T<sub>max</sub>
          </code>
          <b>{shipLive.plan ? `t_f ${fmt(shipLive.plan.final_time, 1)} s` : "no plan"}</b>
        </p>
      </div>
      <div className={css.margins}>
        {lit > 0 && <Margin label="Thrust" value={s.thrust_mag / 1e6} limit={s.thrust_limits[1] / 1e6} unit="MN" digits={2} />}
        {lit > 0 && <Margin label="Gimbal" value={s.gimbal_deg} limit={info.tuning.gimbal_max_deg} unit="°" />}
        <Margin label="Turn rate" value={omega} limit={40} unit="°/s" />
        <Margin label="Tilt" value={s.tilt_deg} limit={s.vehicle === "booster" ? 15 : 90} unit="°" />
      </div>
      <div className={css.graphs}>
        <Graph pick={altitude} label="Altitude" unit="km" scale={0.001} digits={2} />
        <Graph pick={speed} label="Speed" unit="m/s" />
        <Graph pick={thrust} label="Thrust" unit="MN" scale={1e-6} digits={2} />
        <Graph pick={tilt} label="Tilt" unit="°" digits={1} />
        <Graph pick={q} label="Dynamic pressure" unit="kPa" scale={0.001} digits={1} />
        <Graph pick={heat} label="Heating" unit="kW/m²" scale={0.001} digits={1} />
      </div>
    </Panel>
  );
}
