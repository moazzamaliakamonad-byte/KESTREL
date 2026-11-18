"use client";

import { useMemo, useState } from "react";
import { engineLayout } from "@/lib/ship/engines";
import { LINES } from "@/lib/ship/phrases";
import { commentator, failEngine, throwGust } from "@/lib/ship/runtime";
import { ORBIT_LIMITS, PRESETS, SPEEDS, START_LIMITS, useShip, verdict, type CameraMode, type Noise } from "@/lib/ship/store";
import type { EngineState, Preview, Vehicle } from "@/lib/ship/types";
import { Field, Panel, Seg, Slider, Toggle, fmt } from "./controls";
import css from "./console.module.css";

const km = (m: number) => `${(m / 1000).toFixed(m < 10_000 ? 2 : 1)} km`;
const ms = (v: number) => `${fmt(v)} m/s`;
const COMPASS = ["out to sea (+x)", "north-east", "north (+y)", "north-west", "toward the tower (−x)", "south-west", "south (−y)", "south-east"];
const compass = (deg: number) => `${fmt(deg)}° · ${COMPASS[Math.round((((deg % 360) + 360) % 360) / 45) % 8]}`;

/** The engines seen from below. Click one to fail it. */
export function EngineMap({ vehicle, states, failed, onToggle }: { vehicle: Vehicle; states: EngineState[] | null; failed: number[]; onToggle: (i: number) => void }) {
  const spots = useMemo(() => engineLayout(vehicle), [vehicle]);
  return (
    <svg className={css.engineMap} viewBox="-5 -5 10 10" role="group" aria-label="Engines, seen from below">
      <circle r={4.7} className={css.engineHull} />
      {spots.map((e, i) => {
        const state: EngineState | "fixed" = !e.landing ? "fixed" : states?.[i] ?? (failed.includes(i) ? "failed" : "off");
        return (
          <circle
            key={i}
            cx={e.x}
            cy={e.z}
            r={e.r * 0.92}
            data-state={state}
            className={css.engine}
            tabIndex={e.landing ? 0 : -1}
            role={e.landing ? "button" : undefined}
            aria-label={e.landing ? `Engine ${i + 1}: ${state}` : undefined}
            onClick={e.landing ? () => onToggle(i) : undefined}
            onKeyDown={e.landing ? (ev) => (ev.key === "Enter" || ev.key === " ") && onToggle(i) : undefined}
          >
            <title>{e.landing ? `Engine ${i + 1}: ${state}. Click to fail it.` : "Not used for landing"}</title>
          </circle>
        );
      })}
    </svg>
  );
}

/** Side view of the predicted flight: height against downrange. */
function PathMap({ preview }: { preview: Preview | null }) {
  if (!preview?.ok || preview.path.length < 2) return <div className={css.mapEmpty}>No prediction yet</div>;
  const xs = preview.path.map((p) => p[0]);
  const zs = preview.path.map((p) => p[2]);
  const zMax = Math.max(...zs, 1);
  const span = Math.max(Math.max(...xs.map(Math.abs)), zMax * 0.25, 200);
  const X = (x: number) => 100 + (x / span) * 92;
  const Z = (z: number) => 96 - (z / zMax) * 88;
  const d = preview.path.map((p, i) => `${i ? "L" : "M"}${X(p[0]).toFixed(1)},${Z(p[2]).toFixed(1)}`).join("");
  return (
    <svg className={css.map} viewBox="0 0 200 100" preserveAspectRatio="none" aria-label="Predicted path: height against downrange">
      <line x1={0} x2={200} y1={96} y2={96} className={css.mapGround} />
      <line x1={X(-14)} x2={X(-14)} y1={96} y2={Z(145)} className={css.mapTower} />
      <path d={d} className={css.mapPath} data-good={preview.outcome !== "crashed"} />
      {preview.phases
        .filter((p) => p.position)
        .map((p, i) => (
          <circle key={i} cx={X(p.position![0])} cy={Z(p.position![2])} r={1.6} className={css.mapDot}>
            <title>{p.phase}</title>
          </circle>
        ))}
    </svg>
  );
}

function Mission() {
  const { vehicle, scenario, start, setStart, setVehicle, preset, setPreset, preview, previewing, mode, orbit, setOrbit } = useShip();
  const l = START_LIMITS[scenario];
  const locked = mode !== "editing";
  const [p, v] = [start.position, start.velocity];
  const put = (pos: number[], vel: number[]) => setStart({ position: [pos[0], pos[1], pos[2]], velocity: [vel[0], vel[1], vel[2]] });
  return (
    <>
      <Field label="Vehicle">
        <Seg<Vehicle> label="Vehicle" value={vehicle} onChange={setVehicle} disabled={locked} options={[["booster", "Booster"], ["ship", "Ship"]]} />
      </Field>
      <Field label="Start">
        <div className={css.chips}>
          {PRESETS[scenario].map((x, i) => (
            <button key={x.name} aria-pressed={preset === i} disabled={locked} onClick={() => setPreset(i)}>
              <b>{x.name}</b>
              <span>{x.note}</span>
            </button>
          ))}
        </div>
      </Field>
      {orbit ? (
        <>
          <Slider
            label="Distance to the tower at 120 km"
            value={orbit.rangeKm}
            range={ORBIT_LIMITS.rangeKm}
            step={25}
            disabled={locked}
            format={(x) => `${fmt(x)} km`}
            onChange={(x) => setOrbit({ rangeKm: x })}
          />
          <Slider
            label="Entry angle"
            value={orbit.gammaDeg}
            range={ORBIT_LIMITS.gammaDeg}
            step={0.05}
            disabled={locked}
            format={(x) => `${fmt(x, 2)}°`}
            onChange={(x) => setOrbit({ gammaDeg: x })}
          />
          <p className={css.hint}>
            Too shallow and close, or too steep and far, and the ship leaves the entry corridor: it falls short, overheats or overshoots. The prediction flies it to find out.
          </p>
        </>
      ) : (
        <>
          <Slider label="Altitude" value={p[2]} range={l.z} step={100} disabled={locked} format={km} onChange={(z) => put([p[0], p[1], z], v)} />
          <Slider label="Downrange" value={p[0]} range={l.x} step={25} disabled={locked} format={(x) => `${fmt(x)} m`} onChange={(x) => put([x, p[1], p[2]], v)} />
          {l.y[1] > l.y[0] && <Slider label="Crossrange" value={p[1]} range={l.y} step={25} disabled={locked} format={(y) => `${fmt(y)} m`} onChange={(y) => put([p[0], y, p[2]], v)} />}
          <Slider label="Descent speed" value={-v[2]} range={[-l.vz[1], -l.vz[0]]} step={5} disabled={locked} format={ms} onChange={(s) => put(p, [v[0], v[1], -s])} />
          <Slider label="Downrange speed" value={v[0]} range={l.vx} step={1} disabled={locked} format={ms} onChange={(s) => put(p, [s, v[1], v[2]])} />
          {l.vy[1] > l.vy[0] && <Slider label="Crossrange speed" value={v[1]} range={l.vy} step={1} disabled={locked} format={ms} onChange={(s) => put(p, [v[0], s, v[2]])} />}
        </>
      )}
      <Field label="Prediction">
        <PathMap preview={preview} />
        <p className={css.hint} data-testid="prediction">
          {previewing
            ? "Flying it ahead of time…"
            : preview?.ok
              ? `${verdict(scenario, preview.outcome, preview.reason) || "No outcome"} after ${fmt(preview.t)} s${preview.propellant !== undefined && preview.outcome !== "crashed" ? `, ${fmt(preview.propellant / 1000, 1)} t of propellant left` : ""}.${preview.entry ? ` Entry: tiles peak at ${fmt(preview.entry.peak_tile)} K, ${fmt(preview.entry.peak_g, 1)} g.` : ""}`
              : "Prediction unavailable."}
        </p>
      </Field>
    </>
  );
}

function VehicleTab() {
  const { vehicle, scenario, info, options, setOptions, toggleFailed, resetOptions, mode, flight } = useShip();
  const i = info[scenario];
  if (!i) return <p className={css.hint}>Loading the vehicle…</p>;
  const locked = mode !== "editing";
  const standard = i.propellant / 1000;
  const flying = mode === "flying";
  return (
    <>
      <Field label={vehicle === "booster" ? "Landing engines · 13 of 33" : "Landing engines · 3 sea-level"}>
        <div className={css.engineRow}>
          <EngineMap
            vehicle={vehicle}
            states={flying ? flight?.engines ?? null : null}
            failed={options.failed}
            onToggle={(n) => {
              if (flying) failEngine(n);
              else if (!locked) toggleFailed(n);
            }}
          />
          <p className={css.hint}>
            {flying ? "Click an engine to fail it now." : "Click an engine to have it fail to light."}
            <br />
            {options.failed.length ? `${options.failed.length} will not light.` : "All healthy."}
          </p>
        </div>
      </Field>
      <Slider label="Landing propellant" note="propellant" tag="ill" value={options.propellant ?? standard} range={[standard * 0.5, standard * 1.6]} step={1} disabled={locked} format={(t) => `${fmt(t)} t`} onChange={(t) => setOptions({ propellant: t })} />
      <Slider label="Gimbal limit" note="gimbal" tag="ill" value={options.gimbal ?? i.tuning.gimbal_max_deg} range={[4, 20]} step={0.5} disabled={locked} format={(d) => `${fmt(d, 1)}°`} onChange={(d) => setOptions({ gimbal: d })} />
      <Slider label="Minimum throttle" note="throttle" tag="pub" value={(options.throttleMin ?? i.tuning.throttle_min) * 100} range={[20, 80]} step={1} disabled={locked} format={(x) => `${fmt(x)} %`} onChange={(x) => setOptions({ throttleMin: x / 100 })} />
      <Slider label="Specific impulse" note="isp" tag="pub" value={options.isp ?? i.tuning.isp} range={[280, 380]} step={1} disabled={locked} format={(x) => `${fmt(x)} s`} onChange={(x) => setOptions({ isp: x })} />
      <Slider label="Ignition delay" note="ignition" tag="ill" value={options.ignition ?? i.tuning.ignition_time} range={[0.2, 3]} step={0.1} disabled={locked} format={(x) => `${fmt(x, 1)} s`} onChange={(x) => setOptions({ ignition: x })} />
      <Slider label="Throttle lag" note="lag" tag="ill" value={options.lag} range={[0, 2]} step={0.05} format={(x) => `${fmt(x, 2)} s`} onChange={(x) => setOptions({ lag: x })} />
      <button className={css.ghost} disabled={locked} onClick={resetOptions}>
        Restore every default
      </button>
      {locked && <p className={css.hint}>Greyed-out settings apply to the next flight.</p>}
    </>
  );
}

function Environment() {
  const { options, setOptions, mode } = useShip();
  const locked = mode !== "editing";
  return (
    <>
      <Slider label="Steady wind" value={options.windSpeed} range={[0, 30]} step={1} format={ms} onChange={(x) => setOptions({ windSpeed: x })} />
      <Slider label="Wind blows" value={options.windDir} range={[0, 345]} step={15} format={compass} onChange={(x) => setOptions({ windDir: x })} />
      <Slider label="Gust strength" value={options.gustSpeed} range={[0, 40]} step={1} format={ms} onChange={(x) => setOptions({ gustSpeed: x })} />
      <Slider label="Gust blows" value={options.gustDir} range={[0, 345]} step={15} format={compass} onChange={(x) => setOptions({ gustDir: x })} />
      <button
        className={css.action}
        disabled={mode !== "flying"}
        onClick={() => {
          throwGust();
          commentator.say([{ text: LINES.gust }]);
        }}
      >
        Throw a 4-second gust now
      </button>
      <Toggle label="Guidance uses its wind estimate" value={options.windAware} onChange={(x) => setOptions({ windAware: x })} />
      <Slider label="Air density (× standard)" value={options.density} range={[0.5, 1.5]} step={0.05} disabled={locked} format={(x) => `${fmt(x, 2)} ×`} onChange={(x) => setOptions({ density: x })} />
      <Field label="Sensor noise">
        <Seg<Noise> label="Sensor noise" value={options.noise} onChange={(x) => setOptions({ noise: x })} options={[["off", "Off"], ["low", "Low"], ["high", "High"]]} />
      </Field>
      <Field label="Guidance solver">
        <p className={css.hint}>Every plan is solved by Kestrel&apos;s own interior-point solver, written for this project: it allocates no memory while solving and stops after a fixed number of iterations, as flight code needs.</p>
      </Field>
      <Field label="Random seed">
        <Seg<string>
          label="Random seed"
          value={options.seed === null ? "new" : "fixed"}
          onChange={(x) => setOptions({ seed: x === "new" ? null : 7 })}
          options={[
            ["new", "New each flight"],
            ["fixed", "Repeatable"],
          ]}
        />
      </Field>
    </>
  );
}

const CAMERAS: [CameraMode, string][] = [
  ["director", "Director"],
  ["chase", "Chase"],
  ["track", "Tracking"],
  ["onboard", "Onboard"],
  ["tower", "Tower"],
  ["orbit", "Free"],
];

function View() {
  const { view, setView, mode } = useShip();
  return (
    <>
      <Field label="Camera">
        <div className={css.chips} data-compact>
          {CAMERAS.map(([c, name]) => (
            <button key={c} aria-pressed={view.camera === c} onClick={() => setView({ camera: c })}>
              <b>{name}</b>
            </button>
          ))}
        </div>
      </Field>
      <Field label="Simulation speed">
        <Seg<number> label="Simulation speed" value={view.speed} onChange={(x) => setView({ speed: x })} options={SPEEDS.map((s) => [s, `${s}×`])} />
      </Field>
      <Toggle label="Commentary voice" value={view.voice} onChange={(x) => setView({ voice: x })} />
      <Toggle label="Engine and wind sound" value={view.sound} onChange={(x) => setView({ sound: x })} />
      <Toggle label="Predicted path (before a flight)" value={view.preview} onChange={(x) => setView({ preview: x })} />
      <Toggle label="Guidance plan line" value={view.plan} onChange={(x) => setView({ plan: x })} />
      <Toggle label="Flown trail" value={view.trail} onChange={(x) => setView({ trail: x })} />
      <Toggle label="Analysis view" value={view.analysis} onChange={(x) => (mode === "flying" || mode === "done") && setView({ analysis: x, paused: x && mode === "flying" ? true : view.paused })} />
      <p className={css.hint}>
        Keys: <kbd>Space</kbd> fly / pause · <kbd>A</kbd> analysis · <kbd>R</kbd> reset · <kbd>1</kbd>–<kbd>6</kbd> cameras · <kbd>,</kbd> <kbd>.</kbd> speed
      </p>
    </>
  );
}

const TABS = ["Mission", "Vehicle", "Environment", "View"] as const;
type Tab = (typeof TABS)[number];

/** The left dock: everything that can be changed, in four tabs. */
export function Dock({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [tab, setTab] = useState<Tab>("Mission");
  return (
    <Panel className={css.dock} >
      <div className={css.dockInner} data-open={open}>
        <nav className={css.tabs} role="tablist" aria-label="Settings">
          {TABS.map((t) => (
            <button key={t} role="tab" aria-selected={t === tab} onClick={() => setTab(t)}>
              {t}
            </button>
          ))}
          <button className={css.sheetClose} onClick={onClose} aria-label="Close settings">
            ×
          </button>
        </nav>
        <div className={css.tabBody} role="tabpanel">
          {tab === "Mission" && <Mission />}
          {tab === "Vehicle" && <VehicleTab />}
          {tab === "Environment" && <Environment />}
          {tab === "View" && <View />}
        </div>
      </div>
    </Panel>
  );
}
