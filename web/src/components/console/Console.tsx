"use client";

import { shown } from "@/lib/ship/shown";
import { useEffect, useState } from "react";
import { shipLive } from "@/lib/ship/live";
import { PHASE_LABEL, SPEEDS, analysisOn, phasesOf, useShip, verdict, type CameraMode } from "@/lib/ship/store";
import { AnalysisPanel } from "./AnalysisPanel";
import { flightAudio } from "@/lib/ship/audio";
import { commentator, stepShip } from "@/lib/ship/runtime";
import { Dock } from "./Dock";
import { LaunchOverlay, PhaseBanner, launchLines } from "./Launch";
import { Briefing, Telemetry } from "./Telemetry";
import { fmt, useNow, useTick } from "./controls";
import { UnitTip, Units } from "./Units";
import css from "./console.module.css";

const CAMERA_KEYS: CameraMode[] = ["director", "chase", "track", "onboard", "tower", "orbit"];
const CAMERA_NAME: Record<CameraMode, string> = { director: "Director", chase: "Chase", track: "Tracking", onboard: "Onboard", tower: "Tower", orbit: "Free" };

/** `mm:ss.s` (or `mm:ss` when `whole`). */
function clock(t: number, whole = false): string {
  const a = Math.abs(t);
  const m = Math.floor(a / 60);
  const s = a - m * 60;
  return `${String(m).padStart(2, "0")}:${whole ? String(Math.floor(s)).padStart(2, "0") : s.toFixed(1).padStart(4, "0")}`;
}

/** The mark: a kestrel hovering, wings spread and tail fanned, seen from above. */
function Mark() {
  return (
    <svg viewBox="0 0 24 24" className={css.mark} aria-hidden>
      <path d="M12 3.2C12.9 3.2 13.5 4 13.4 5.1L13.3 6.2C16 6.3 19.5 7.6 22.5 10.6C19.6 10.1 17.4 10.4 15.6 11.4C14.6 12 14 13 13.7 14.2L15.6 20.4L12 18.9L8.4 20.4L10.3 14.2C10 13 9.4 12 8.4 11.4C6.6 10.4 4.4 10.1 1.5 10.6C4.5 7.6 8 6.3 10.7 6.2L10.6 5.1C10.5 4 11.1 3.2 12 3.2Z" />
    </svg>
  );
}

function TopBar({ onControls, controls }: { onControls: () => void; controls: boolean }) {
  useTick(100);
  const { scenario, flight, mode, view, setView } = useShip();
  const phases = phasesOf(scenario);
  const now = mode === "editing" || mode === "launch" ? -1 : phases.indexOf(flight?.phase ?? phases[0]);
  // During the opening sequence the clock is the sequence's own (T− on the
  // pad, T+ after liftoff); in flight it is the simulation's time.
  const launch = mode === "launch" ? shipLive.launchClock : null;
  const t = launch ?? shipLive.snapshot?.t ?? 0;
  // A count shows the second it is in: T− 00:10 until the "nine" is spoken.
  const text = launch !== null && launch < 0 ? `T− ${clock(Math.ceil(-launch), true)}` : `T+ ${clock(t, launch !== null)}`;
  const flying = mode === "flying" || mode === "done";
  return (
    <header className={css.top}>
      <div className={css.brand}>
        <Mark />
        <div>
          <b>KESTREL</b>
          <span>Starship-class flight simulator</span>
        </div>
      </div>
      <ol className={css.timeline} aria-label="Flight phases">
        {phases.slice(0, -1).map((p, i) => (
          <li key={p} data-state={i < now || flight?.outcome ? "done" : i === now ? "now" : "next"}>
            <i />
            {PHASE_LABEL[p]}
          </li>
        ))}
      </ol>
      <div className={css.status}>
        <span className={css.clock} data-testid="clock">
          {text}
        </span>
        {view.paused && <span className={css.chip} data-tone="warm">Paused</span>}
        <span className={css.chip}>Simulation</span>
        <button className={css.icon} aria-pressed={view.voice} onClick={() => setView({ voice: !view.voice })} title="Commentary voice (V)">
          {view.voice ? "Voice on" : "Voice off"}
        </button>
        <button className={`${css.icon} ${flying ? "" : css.onlyPhone}`} aria-pressed={controls} onClick={onControls} title="Settings (C)">
          Settings
        </button>
        <button className={css.icon} aria-pressed={view.help} onClick={() => setView({ help: !view.help })} title="Keyboard shortcuts (?)" aria-label="Keyboard shortcuts">
          ?
        </button>
      </div>
    </header>
  );
}

/** Start the flight (the opening sequence first) and, being a click, the audio. */
function go() {
  flightAudio.start();
  commentator.load();
  // The countdown's clips must start on their second: fetch them now.
  commentator.preload(launchLines());
  useShip.getState().fly();
}

function toggleAnalysis() {
  const s = useShip.getState();
  if (s.mode !== "flying" && s.mode !== "done") return;
  s.setView({ analysis: !s.view.analysis, paused: !s.view.analysis && s.mode === "flying" ? true : s.view.paused });
}

function ActionBar() {
  const { mode, view, setView, reset, info, scenario, previewing, beginFlight } = useShip();
  const ready = !!info[scenario];
  const flying = mode === "flying";
  const i = SPEEDS.indexOf(view.speed as (typeof SPEEDS)[number]);
  const analysis = analysisOn({ view, mode });
  return (
    <div className={css.actions}>
      <div className={css.stepper} role="group" aria-label="Camera" data-off={analysis || undefined}>
        <button onClick={() => setView({ camera: CAMERA_KEYS[(CAMERA_KEYS.indexOf(view.camera) + CAMERA_KEYS.length - 1) % CAMERA_KEYS.length] })} aria-label="Previous camera">
          ‹
        </button>
        <span>
          <em>Camera</em>
          {analysis ? "Free" : CAMERA_NAME[view.camera]}
        </span>
        <button onClick={() => setView({ camera: CAMERA_KEYS[(CAMERA_KEYS.indexOf(view.camera) + 1) % CAMERA_KEYS.length] })} aria-label="Next camera">
          ›
        </button>
      </div>
      {mode === "editing" ? (
        <button className={css.primary} disabled={!ready} onClick={go} data-busy={previewing || undefined}>
          Fly
          <kbd>Space</kbd>
        </button>
      ) : mode === "launch" ? (
        <>
          <button className={css.secondary} onClick={reset}>
            Cancel<kbd>R</kbd>
          </button>
          <button className={css.primary} onClick={beginFlight}>
            Skip to the return<kbd>Space</kbd>
          </button>
        </>
      ) : (
        <>
          <button className={css.secondary} onClick={reset}>
            Reset<kbd>R</kbd>
          </button>
          {flying ? (
            <button className={css.primary} onClick={() => setView({ paused: !view.paused })}>
              {view.paused ? "Resume" : "Pause"}
              <kbd>Space</kbd>
            </button>
          ) : (
            <button className={css.primary} onClick={reset}>
              Fly again<kbd>Space</kbd>
            </button>
          )}
          <button className={css.secondary} aria-pressed={view.analysis} data-glow={view.paused && !view.analysis ? true : undefined} onClick={toggleAnalysis}>
            {view.analysis ? "Back to the flight" : "Analyze"}
            <kbd>A</kbd>
          </button>
        </>
      )}
      <div className={css.stepper} role="group" aria-label="Simulation speed">
        <button disabled={i <= 0} onClick={() => setView({ speed: SPEEDS[i - 1] })} aria-label="Slower">
          −
        </button>
        <span>
          <em>Speed</em>
          {view.speed}×
        </span>
        <button disabled={i >= SPEEDS.length - 1} onClick={() => setView({ speed: SPEEDS[i + 1] })} aria-label="Faster">
          +
        </button>
      </div>
    </div>
  );
}

function Caption() {
  const now = useNow(200);
  const c = shipLive.caption;
  const fresh = c && now - c.at < 5000;
  return (
    <div className={css.caption} data-show={fresh || undefined} aria-live="polite">
      {c?.text}
    </div>
  );
}

/** The flight's numbers, from its recorded history and its last instant. */
function Report() {
  const s = shown();
  const h = shipLive.history;
  if (!s || h.length < 2) return null;
  const peak = (pick: (x: (typeof h)[number]) => number) => h.reduce((m, x) => Math.max(m, pick(x)), 0);
  const end = s.end_velocity;
  const rows: [string, string][] = [
    ["Flight time", `${fmt(s.t, 1)} s`],
    ["Propellant left", `${fmt(s.propellant / 1000, 1)} t`],
    ["Guidance plans", String(shipLive.replans)],
    ["Fastest", `${fmt(peak((x) => x.speed))} m/s · Mach ${fmt(peak((x) => x.mach), 2)}`],
    ["Hardest braking", `${fmt(peak((x) => x.g), 2)} g`],
    ["Peak air pressure", `${fmt(peak((x) => x.q) / 1000, 1)} kPa`],
  ];
  if (end) rows.push(["At contact", `${fmt(-end[2], 2)} m/s down · ${fmt(Math.hypot(end[0], end[1]), 2)} m/s sideways`]);
  rows.push(["Off the catch axis", `${fmt(Math.hypot(s.position[0], s.position[1]), 2)} m`]);
  return (
    <dl className={css.report}>
      {rows.map(([k, v]) => (
        <div key={k}>
          <dt>{k}</dt>
          <dd>
            <Units text={v} />
          </dd>
        </div>
      ))}
    </dl>
  );
}

function Verdict() {
  const { mode, scenario, flight, reset, setView, view, continueWithShip, parked } = useShip();
  if (mode !== "done" || !flight?.outcome || view.analysis) return null;
  const good = flight.outcome !== "crashed";
  return (
    <div className={css.verdict} data-good={good} role="status">
      <span>{good ? (parked ? "Both stages recovered" : "Mission complete") : "Flight ended"}</span>
      <h1 data-testid="verdict">{verdict(scenario, flight.outcome, flight.reason)}</h1>
      <Report />
      {scenario === "booster" && flight.outcome === "caught" && <p className={css.next}>The ship is still up there. Fly its return to the same tower, or end the mission here.</p>}
      <div className={css.verdictActions}>
        {scenario === "booster" && flight.outcome === "caught" ? (
          <>
            <button className={css.primary} onClick={continueWithShip}>
              Fly the ship&apos;s return
            </button>
            <button className={css.secondary} onClick={reset}>
              End mission
            </button>
          </>
        ) : (
          <button className={css.primary} onClick={reset}>
            Fly again
          </button>
        )}
        <button className={css.secondary} onClick={() => setView({ analysis: true })}>
          Analyze the flight
        </button>
      </div>
    </div>
  );
}

const KEYS: [string, string][] = [
  ["Space", "Fly · pause and resume · skip the launch"],
  ["A", "Analysis view: the blueprint and its calculations"],
  ["N", "While paused: step the flight one second"],
  ["Esc", "Close the open part, then the analysis view"],
  ["R", "Reset to the pad"],
  ["1 – 6", "Cameras: director, chase, tracking, onboard, tower, free"],
  [",  .", "Slower · faster"],
  ["C", "Settings (vehicle, wind, failures) during a flight"],
  ["V", "Commentary voice on or off"],
  ["?", "This sheet"],
];

function Help() {
  const { view, setView } = useShip();
  if (!view.help) return null;
  return (
    <div className={css.help} role="dialog" aria-label="Keyboard shortcuts" onClick={() => setView({ help: false })}>
      <section onClick={(e) => e.stopPropagation()}>
        <header>
          <h2>Keyboard</h2>
          <button onClick={() => setView({ help: false })} aria-label="Close">
            ×
          </button>
        </header>
        <dl>
          {KEYS.map(([k, v]) => (
            <div key={k}>
              <dt>
                <kbd>{k}</kbd>
              </dt>
              <dd>
            <Units text={v} />
          </dd>
            </div>
          ))}
        </dl>
        <p>In the analysis view: click a part of the drawing to open it, drag to turn the vehicle, scroll to zoom. Clicking an engine in the engine map fails it.</p>
      </section>
    </div>
  );
}

/** The mission-control console around the 3D view. */
export function Console() {
  const { mode, error, view } = useShip();
  // Settings: always there before a flight, tucked away during one.
  const [controls, setControls] = useState(false);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT")) return;
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      const s = useShip.getState();
      const key = e.key.toLowerCase();
      if (e.code === "Space") {
        e.preventDefault();
        if (s.mode === "editing") go();
        else if (s.mode === "launch") s.beginFlight();
        else if (s.mode === "flying") s.setView({ paused: !s.view.paused });
        else s.reset();
      } else if (e.key === "Escape") {
        if (s.view.help) s.setView({ help: false });
        else if (s.view.part) s.setView({ part: null });
        else if (s.view.analysis) s.setView({ analysis: false });
        else setControls(false);
      } else if (key === "r") s.reset();
      else if (key === "a") toggleAnalysis();
      else if (key === "n") stepShip(1);
      else if (key === "c") setControls((v) => !v);
      else if (key === "v") s.setView({ voice: !s.view.voice });
      else if (e.key === "?" || key === "h") s.setView({ help: !s.view.help });
      else if (e.key >= "1" && e.key <= "6") s.setView({ camera: CAMERA_KEYS[Number(e.key) - 1] });
      else if (e.key === "," || e.key === ".") {
        const i = SPEEDS.indexOf(s.view.speed as (typeof SPEEDS)[number]) + (e.key === "," ? -1 : 1);
        if (i >= 0 && i < SPEEDS.length) s.setView({ speed: SPEEDS[i] });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const analysis = analysisOn({ view, mode });
  const editing = mode === "editing";
  const dock = !analysis && (editing || controls);
  return (
    <div className={css.hud} data-analysis={analysis || undefined} data-mode={mode} data-clean={(!editing && !dock && !analysis) || undefined}>
      <div className={css.frame} aria-hidden />
      <TopBar controls={controls} onControls={() => setControls((v) => !v)} />
      <aside className={css.left} data-sheet={controls || undefined}>
        {analysis ? <AnalysisPanel /> : dock || editing ? <Dock open={controls} onClose={() => setControls(false)} /> : null}
      </aside>
      <aside className={css.right}>{editing || mode === "launch" ? <Briefing /> : <Telemetry />}</aside>
      <PhaseBanner />
      <LaunchOverlay />
      <footer className={css.bottom}>
        <Caption />
        <ActionBar />
      </footer>
      <Verdict />
      <Help />
      <UnitTip />
      {error && (
        <div className={css.error} role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
