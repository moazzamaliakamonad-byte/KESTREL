"use client";

import { useEffect, useState } from "react";
import { currentShip } from "@/lib/ship/runtime";
import { shipLive } from "@/lib/ship/live";
import { PHASE_LABEL, START_LIMITS, useShip, verdict, type CameraMode, type Noise } from "@/lib/ship/store";
import type { ScenarioKind } from "@/lib/ship/types";
import css from "./ship.module.css";

function useTick(ms: number) {
  const [, set] = useState(0);
  useEffect(() => {
    const id = setInterval(() => set((n) => n + 1), ms);
    return () => clearInterval(id);
  }, [ms]);
}

function Seg<T extends string | number>({ value, options, onChange, label }: { value: T; options: [T, string][]; onChange: (v: T) => void; label: string }) {
  return (
    <div className={css.seg} role="group" aria-label={label}>
      {options.map(([v, text]) => (
        <button key={String(v)} aria-pressed={v === value} onClick={() => onChange(v)}>
          {text}
        </button>
      ))}
    </div>
  );
}

function Slider({ label, unit, value, range, step, onChange }: { label: string; unit: string; value: number; range: [number, number]; step: number; onChange: (v: number) => void }) {
  return (
    <label className={css.slider}>
      <span>
        {label} <b>{Math.round(value).toLocaleString()} {unit}</b>
      </span>
      <input type="range" min={range[0]} max={range[1]} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}

/** Start editor (before a flight): scenario, altitude, downrange, speeds. */
function StartPanel() {
  const { scenario, start, setStart, setScenario, preview, previewing } = useShip();
  const l = START_LIMITS[scenario];
  const [p, v] = [start.position, start.velocity];
  const set = (pos: number[], vel: number[]) => setStart({ position: [pos[0], 0, pos[2]], velocity: [vel[0], 0, vel[2]] });
  return (
    <section className={css.panel}>
      <h2>Mission</h2>
      <Seg<ScenarioKind>
        label="Scenario"
        value={scenario}
        onChange={setScenario}
        options={[
          ["earth", "Earth · tower catch"],
          ["mars", "Mars · legs"],
        ]}
      />
      <Slider label="Altitude" unit="m" value={p[2]} range={l.z} step={50} onChange={(z) => set([p[0], 0, z], v)} />
      <Slider label="Downrange" unit="m" value={p[0]} range={l.x} step={25} onChange={(x) => set([x, 0, p[2]], v)} />
      <Slider label="Fall speed" unit="m/s" value={-v[2]} range={[-l.vz[1], -l.vz[0]]} step={1} onChange={(s) => set(p, [v[0], 0, -s])} />
      <Slider label="Sideways speed" unit="m/s" value={v[0]} range={l.vx} step={1} onChange={(s) => set(p, [s, 0, v[2]])} />
      <p className={css.hint}>
        {previewing ? "Predicting the flight…" : preview?.ok ? `Prediction: ${verdict(scenario, preview.outcome, preview.reason) || "—"} after ${preview.t.toFixed(0)} s` : "Prediction unavailable"}
      </p>
    </section>
  );
}

/** Break it: disturbances during the flight. */
function BreakPanel() {
  const { lag, noise, speed, camera, set } = useShip();
  const [msg, setMsg] = useState("");
  const act = (f: () => boolean | void, text: string) => {
    const ok = f();
    setMsg(ok === false ? "No more engines to fail" : text);
  };
  return (
    <section className={css.panel}>
      <h2>Break it</h2>
      <div className={css.row}>
        <button className={css.hit} onClick={() => act(() => currentShip()?.gust(-1), "Gust toward the tower")}>Gust ←</button>
        <button className={css.hit} onClick={() => act(() => currentShip()?.gust(1), "Gust out to sea")}>Gust →</button>
        <button className={`${css.hit} ${css.danger}`} onClick={() => act(() => currentShip()?.engine_out() ?? false, "Engine failed")}>Engine out</button>
      </div>
      <span className={css.label}>Throttle lag</span>
      <Seg<number> label="Throttle lag" value={lag} onChange={(v) => set({ lag: v })} options={[[0, "0 s"], [0.3, "0.3 s"], [1, "1 s"]]} />
      <span className={css.label}>Sensor noise</span>
      <Seg<Noise> label="Sensor noise" value={noise} onChange={(v) => set({ noise: v })} options={[["off", "Off"], ["low", "Low"], ["high", "High"]]} />
      <span className={css.label}>Speed</span>
      <Seg<number> label="Speed" value={speed} onChange={(v) => set({ speed: v })} options={[[1, "1×"], [2, "2×"], [4, "4×"]]} />
      <span className={css.label}>Camera</span>
      <Seg<CameraMode> label="Camera" value={camera} onChange={(v) => set({ camera: v })} options={[["chase", "Chase"], ["tower", "Ground"], ["orbit", "Free"]]} />
      {msg && <p className={css.hint}>{msg}</p>}
    </section>
  );
}

const fmt = (v: number, d = 0) => (Number.isFinite(v) ? v.toFixed(d) : "—");

/** Live numbers, refreshed ~10× a second from `shipLive`. */
function Telemetry() {
  useTick(100);
  const view = useShip((s) => s.view);
  const s = shipLive.snapshot;
  if (!s) return null;
  const speed = Math.hypot(...s.velocity);
  const solve = shipLive.solveMs.at(-1);
  return (
    <section className={`${css.panel} ${css.telemetry}`}>
      <h2>{PHASE_LABEL[s.phase]}</h2>
      <div className={css.grid}>
        <span>Altitude</span><b>{fmt(s.position[2])} m</b>
        <span>Speed</span><b>{fmt(speed)} m/s</b>
        <span>Down</span><b>{fmt(-s.velocity[2], 1)} m/s</b>
        <span>Tilt</span><b>{fmt(s.tilt_deg)}°</b>
        <span>Propellant</span><b>{fmt(s.propellant / 1000, 1)} t</b>
        <span>Thrust</span><b>{fmt(s.thrust_mag / 1e6, 2)} MN</b>
        <span>Gimbal</span><b>{fmt(s.gimbal_deg, 1)}°</b>
        <span>Dyn. pressure</span><b>{fmt(s.q_dyn / 1000, 1)} kPa</b>
        <span>Guidance</span>
        <b>{view?.guide === "plan" ? `SCvx · ${view.legEngines} eng` : view?.guide === "brake" ? "braking" : "—"}</b>
        <span>Last solve</span><b>{solve !== undefined ? `${fmt(solve)} ms` : "—"}</b>
      </div>
      <div className={css.engines} aria-label="Engines">
        {s.engines.map((e, i) => (
          <span key={i} data-state={e} title={`Engine ${i + 1}: ${e}`}>
            R{i + 1}
          </span>
        ))}
      </div>
      <div className={css.flaps} aria-label="Flaps">
        {s.flaps.map((f, i) => (
          <span key={i} title={["fore left", "fore right", "aft left", "aft right"][i]}>
            <i style={{ height: `${Math.round(f * 100)}%` }} />
          </span>
        ))}
        <em>flaps</em>
        <em className={s.rcs.some(Boolean) ? css.on : ""}>RCS</em>
      </div>
    </section>
  );
}

function Verdict() {
  const { mode, scenario, view, reset } = useShip();
  if (mode !== "done" || !view?.outcome) return null;
  const good = view.outcome !== "crashed";
  const s = shipLive.snapshot;
  return (
    <div className={css.verdict} data-good={good}>
      <h1>{verdict(scenario, view.outcome, view.reason)}</h1>
      {s && (
        <p>
          {fmt(s.t)} s · {fmt(s.propellant / 1000, 1)} t propellant left · {shipLive.replans} SCvx plans
        </p>
      )}
      <button className={css.land} onClick={reset}>Fly again</button>
    </div>
  );
}

export function ShipHud() {
  const { mode, land, reset, error, previewing, info, scenario } = useShip();
  const ready = !!info[scenario];
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.code !== "Space" || e.target instanceof HTMLInputElement) return;
      e.preventDefault();
      if (useShip.getState().mode === "editing") useShip.getState().land();
      else useShip.getState().reset();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  return (
    <div className={css.hud}>
      <header className={css.brand}>
        <b>KESTREL</b>
        <span>6-DoF ship landing · successive convexification · simulation only</span>
      </header>
      <div className={css.left}>{mode === "editing" ? <StartPanel /> : <BreakPanel />}</div>
      <div className={css.right}>{mode !== "editing" && <Telemetry />}</div>
      <div className={css.bottom}>
        {mode === "editing" ? (
          <button className={css.land} disabled={!ready || previewing} onClick={land}>
            Fly <kbd>Space</kbd>
          </button>
        ) : (
          <button className={css.land} onClick={reset}>
            Reset <kbd>Space</kbd>
          </button>
        )}
      </div>
      <Verdict />
      {error && <div className={css.error}>{error}</div>}
    </div>
  );
}
