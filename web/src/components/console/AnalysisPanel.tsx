"use client";

import { shown } from "@/lib/ship/shown";
import { useState } from "react";
import { shipLive, type Sample } from "@/lib/ship/live";
import type { Decision } from "@/lib/ship/record";
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
const KIND_LABEL: Record<Decision["kind"], string> = {
  phase: "Phase",
  ignition: "Ignition",
  plan: "Guidance",
  "plan-failed": "Guidance",
  engines: "Engines",
  failure: "Failure",
  relight: "Recovery",
  arms: "Tower",
  outcome: "Outcome",
  event: "You",
};

/** The flight's timeline: drag to replay any recorded instant; a mark per decision. */
function Scrubber() {
  useTick(250);
  const scrubT = useShip((s) => s.view.scrubT);
  const mode = useShip((s) => s.mode);
  const paused = useShip((s) => s.view.paused);
  const setView = useShip((s) => s.setView);
  const frames = shipLive.frames;
  if (frames.length < 2) return null;
  const t0 = frames[0].t;
  const t1 = frames[frames.length - 1].t;
  const can = mode === "done" || paused;
  const t = scrubT ?? t1;
  const pos = (x: number) => `${((x - t0) / Math.max(1e-6, t1 - t0)) * 100}%`;
  return (
    <div className={css.scrub} data-live={scrubT === null || undefined}>
      <div className={css.scrubHead}>
        <span>{scrubT === null ? "Live" : "Replaying"} · T+ {fmt(t, 1)} s</span>
        {scrubT !== null && <button onClick={() => setView({ scrubT: null })}>Back to the end</button>}
        {!can && <em>Pause to replay</em>}
      </div>
      <div className={css.scrubTrack}>
        {shipLive.decisions.map((d, i) => (
          <i key={i} data-kind={d.kind} style={{ left: pos(d.t) }} title={`${fmt(d.t, 1)} s · ${d.title}`} />
        ))}
        <input
          type="range"
          min={t0}
          max={t1}
          step={0.1}
          value={t}
          disabled={!can}
          aria-label="Replay the flight at a time"
          onChange={(e) => setView({ scrubT: Number(e.target.value) >= t1 - 0.05 ? null : Number(e.target.value) })}
        />
      </div>
    </div>
  );
}

/** Every decision of the flight with its reason and numbers; click one to replay that instant. */
function Decisions() {
  useTick(400);
  const scrubT = useShip((s) => s.view.scrubT);
  const mode = useShip((s) => s.mode);
  const paused = useShip((s) => s.view.paused);
  const setView = useShip((s) => s.setView);
  const [open, setOpen] = useState<number | null>(null);
  const list = shipLive.decisions;
  const can = mode === "done" || paused;
  // The decision in force at the replayed instant.
  const at = scrubT ?? Infinity;
  let current = -1;
  list.forEach((d, i) => {
    if (d.t <= at) current = i;
  });
  if (!list.length) return <p className={css.hint}>No decisions yet.</p>;
  return (
    <ol className={css.decisions}>
      {list.map((d, i) => (
        <li key={i} data-kind={d.kind} data-current={i === current || undefined} data-open={open === i || undefined}>
          <button
            onClick={() => {
              setOpen(open === i ? null : i);
              if (can) setView({ scrubT: d.t });
            }}
          >
            <time>{fmt(d.t, 1)} s</time>
            <em>{KIND_LABEL[d.kind]}</em>
            <b>{d.title}</b>
          </button>
          {open === i && (
            <div className={css.decisionBody}>
              <p>{d.why}</p>
              {d.facts.length > 0 && (
                <dl>
                  {d.facts.map(([k, v]) => (
                    <div key={k}>
                      <dt>{k}</dt>
                      <dd>{v}</dd>
                    </div>
                  ))}
                </dl>
              )}
            </div>
          )}
        </li>
      ))}
    </ol>
  );
}

export function AnalysisPanel() {
  useTick(150);
  const info = useShip((s) => s.info[s.scenario]);
  const vehicle = useShip((s) => s.vehicle);
  const part = useShip((s) => s.view.part);
  const mode = useShip((s) => s.mode);
  const paused = useShip((s) => s.view.paused);
  const setView = useShip((s) => s.setView);
  // After the flight the record is the story: open on the decisions.
  const [tab, setTab] = useState<"parts" | "decisions">(mode === "done" ? "decisions" : "parts");
  const s = shown();
  if (!s || !info) return null;
  const parts = partsOf(vehicle);
  const sheet = partSheet(part ?? "avionics", s, info, { plan: shipLive.plan, solveMs: shipLive.solveMs.at(-1), replans: shipLive.replans });
  const stepping = mode === "flying" && paused && !s.outcome;
  const decisions = tab === "decisions";
  return (
    <Panel className={css.analysis}>
      <nav className={css.analysisTabs} role="tablist" aria-label="Analysis">
        <button role="tab" aria-selected={!decisions} onClick={() => setTab("parts")}>
          Calculations
        </button>
        <button role="tab" aria-selected={decisions} onClick={() => setTab("decisions")}>
          Decisions · {shipLive.decisions.length}
        </button>
      </nav>
      <Scrubber />
      {decisions ? (
        <Decisions />
      ) : (
        <>
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
        </>
      )}
      <p className={css.hint}>Every number is the simulator&apos;s at this instant, and every formula is the one it integrates. Simulation only.</p>
    </Panel>
  );
}
