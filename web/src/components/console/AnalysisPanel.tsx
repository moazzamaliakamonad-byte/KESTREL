"use client";

import { shipLive, type Sample } from "@/lib/ship/live";
import { partSheet, partsOf, type Calc } from "@/lib/ship/parts";
import { stepShip } from "@/lib/ship/runtime";
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
const propellant = (s: Sample) => s.propellant;
const g = (s: Sample) => s.g;

/** Which graphs belong with which part. */
function graphsFor(part: string | null) {
  if (part === "engines") return [<Graph key="t" pick={thrust} label="Thrust" unit="MN" scale={1e-6} digits={2} />, <Graph key="g" pick={g} label="Thrust acceleration" unit="g" digits={2} />];
  if (part === "tanks") return [<Graph key="p" pick={propellant} label="Propellant" unit="t" scale={0.001} digits={1} />, <Graph key="t" pick={thrust} label="Thrust" unit="MN" scale={1e-6} digits={2} />];
  if (part === "shield") return [<Graph key="q" pick={q} label="Dynamic pressure" unit="kPa" scale={0.001} digits={1} />, <Graph key="h" pick={heat} label="Heating" unit="kW/m²" scale={0.001} digits={1} />];
  if (part?.startsWith("flap") || part?.startsWith("fin")) return [<Graph key="q" pick={q} label="Dynamic pressure" unit="kPa" scale={0.001} digits={1} />, <Graph key="tl" pick={tilt} label="Tilt" unit="°" digits={1} />];
  if (part === "pins") return [<Graph key="a" pick={altitude} label="Altitude" unit="km" scale={0.001} digits={2} />, <Graph key="tl" pick={tilt} label="Tilt" unit="°" digits={1} />];
  if (part === "rcs") return [<Graph key="tl" pick={tilt} label="Tilt" unit="°" digits={1} />, <Graph key="q" pick={q} label="Dynamic pressure" unit="kPa" scale={0.001} digits={1} />];
  return [
    <Graph key="a" pick={altitude} label="Altitude" unit="km" scale={0.001} digits={2} />,
    <Graph key="s" pick={speed} label="Speed" unit="m/s" />,
    <Graph key="t" pick={thrust} label="Thrust" unit="MN" scale={1e-6} digits={2} />,
    <Graph key="tl" pick={tilt} label="Tilt" unit="°" digits={1} />,
  ];
}

function Row({ c }: { c: Calc }) {
  return (
    <div className={css.calc} data-tight={(c.share ?? 0) > 0.9 || undefined}>
      <span>{c.label}</span>
      <b>{c.value}</b>
      {c.formula && <code>{c.formula}</code>}
      {c.share !== undefined && (
        <div className={css.calcBar}>
          <i style={{ width: `${c.share * 100}%` }} />
          {c.limit && <em>{c.limit}</em>}
        </div>
      )}
    </div>
  );
}

/**
 * The analysis view's panel. The vehicle is a drawing now: this lists its
 * parts, and for the open one (or the whole vehicle) shows the formulas
 * the simulator integrates there with this instant's numbers in them, how
 * close each is to its limit, and the history behind it. While the flight
 * is paused it can be stepped forward a tenth of a second or a second at
 * a time.
 */
export function AnalysisPanel() {
  useTick(150);
  const info = useShip((s) => s.info[s.scenario]);
  const vehicle = useShip((s) => s.vehicle);
  const part = useShip((s) => s.view.part);
  const mode = useShip((s) => s.mode);
  const paused = useShip((s) => s.view.paused);
  const setView = useShip((s) => s.setView);
  const s = shipLive.snapshot;
  if (!s || !info) return null;
  const parts = partsOf(vehicle);
  const sheet = partSheet(part ?? "avionics", s, info, { plan: shipLive.plan, solveMs: shipLive.solveMs.at(-1), replans: shipLive.replans });
  const stepping = mode === "flying" && paused && !s.outcome;
  return (
    <Panel className={css.analysis}>
      <header className={css.sheetHead}>
        <span>Analysis · T+ {fmt(s.t, 1)} s</span>
        <h2>{part ? sheet.title : "Whole vehicle"}</h2>
        <p>{part ? sheet.blurb : "Click any part of the drawing, or pick one here, to open its calculations. Drag to turn the vehicle, scroll to zoom."}</p>
      </header>
      <nav className={css.partNav} aria-label="Parts">
        <button aria-pressed={part === null} onClick={() => setView({ part: null })}>
          Whole vehicle
        </button>
        {parts.map((p) => (
          <button key={p.id} aria-pressed={part === p.id} onClick={() => setView({ part: p.id })}>
            {p.short}
          </button>
        ))}
      </nav>
      <div className={css.calcs}>
        {sheet.calcs.map((c) => (
          <Row key={c.label} c={c} />
        ))}
      </div>
      {mode === "flying" && (
        <div className={css.stepRow}>
          <span>{stepping ? "Paused · step the flight" : s.outcome ? "Flight over" : "Running"}</span>
          <button disabled={!stepping} onClick={() => stepShip(0.1)}>
            +0.1 s
          </button>
          <button disabled={!stepping} onClick={() => stepShip(1)}>
            +1 s<kbd>N</kbd>
          </button>
          <button onClick={() => setView({ paused: !paused })}>{paused ? "Run" : "Pause"}</button>
        </div>
      )}
      <div className={css.graphs}>{graphsFor(part)}</div>
      <p className={css.hint}>Every number is the simulator&apos;s at this instant, and every formula is the one it integrates. Simulation only.</p>
    </Panel>
  );
}
