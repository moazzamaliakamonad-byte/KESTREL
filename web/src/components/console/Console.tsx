"use client";

import { useEffect, useState } from "react";
import { shipLive } from "@/lib/ship/live";
import { PHASE_LABEL, SPEEDS, phasesOf, useShip, verdict, type CameraMode } from "@/lib/ship/store";
import { AnalysisPanel } from "./AnalysisPanel";
import { flightAudio } from "@/lib/ship/audio";
import { commentator } from "@/lib/ship/runtime";
import { Dock } from "./Dock";
import { AnalysisIntro, LaunchOverlay, PhaseBanner } from "./Launch";
import { Briefing, Telemetry } from "./Telemetry";
import { fmt, useNow, useTick } from "./controls";
import css from "./console.module.css";

const CAMERA_KEYS: CameraMode[] = ["director", "chase", "track", "onboard", "tower", "orbit"];
const CAMERA_NAME: Record<CameraMode, string> = { director: "Director", chase: "Chase", track: "Tracking", onboard: "Onboard", tower: "Tower", orbit: "Free" };

function clock(t: number): string {
  const m = Math.floor(t / 60);
  const s = t - m * 60;
  return `${String(m).padStart(2, "0")}:${s.toFixed(1).padStart(4, "0")}`;
}

function Mark() {
  return (
    <svg viewBox="0 0 24 24" className={css.mark} aria-hidden>
      <path d="M12 2 L20 20 L12 15 L4 20 Z" />
    </svg>
  );
}

function TopBar({ onSettings }: { onSettings: () => void }) {
  useTick(100);
  const { scenario, flight, mode, view, setView } = useShip();
  const phases = phasesOf(scenario);
  const now = mode === "editing" || mode === "launch" ? -1 : phases.indexOf(flight?.phase ?? phases[0]);
  const t = shipLive.snapshot?.t ?? 0;
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
          T+ {clock(t)}
        </span>
        {view.paused && <span className={css.chip} data-tone="warm">Paused</span>}
        <span className={css.chip}>Simulation</span>
        <button className={css.icon} aria-pressed={view.voice} onClick={() => setView({ voice: !view.voice })} title="Commentary voice">
          {view.voice ? "Voice on" : "Voice off"}
        </button>
        <button className={`${css.icon} ${css.onlyPhone}`} onClick={onSettings}>
          Settings
        </button>
      </div>
    </header>
  );
}

/** Start the flight (the opening sequence first) and, being a click, the audio. */
function go() {
  flightAudio.start();
  commentator.load();
  useShip.getState().fly();
}

function ActionBar() {
  const { mode, view, setView, reset, info, scenario, previewing, beginFlight } = useShip();
  const ready = !!info[scenario];
  const flying = mode === "flying";
  const i = SPEEDS.indexOf(view.speed as (typeof SPEEDS)[number]);
  return (
    <div className={css.actions}>
      <div className={css.stepper} role="group" aria-label="Camera">
        <button onClick={() => setView({ camera: CAMERA_KEYS[(CAMERA_KEYS.indexOf(view.camera) + CAMERA_KEYS.length - 1) % CAMERA_KEYS.length] })} aria-label="Previous camera">
          ‹
        </button>
        <span>
          <em>Camera</em>
          {CAMERA_NAME[view.camera]}
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
          <button className={css.secondary} aria-pressed={view.analysis} data-glow={view.paused && !view.analysis ? true : undefined} onClick={() => setView({ analysis: !view.analysis, paused: !view.analysis && flying ? true : view.paused })}>
            Analyze<kbd>A</kbd>
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

function Verdict() {
  const { mode, scenario, flight, reset, setView, view, continueWithShip, parked } = useShip();
  if (mode !== "done" || !flight?.outcome || view.analysis) return null;
  const good = flight.outcome !== "crashed";
  const s = shipLive.snapshot;
  return (
    <div className={css.verdict} data-good={good} role="status">
      <span>{good ? (parked ? "Both stages recovered" : "Mission complete") : "Flight ended"}</span>
      <h1 data-testid="verdict">{verdict(scenario, flight.outcome, flight.reason)}</h1>
      {s && (
        <p>
          {fmt(s.t)} s of flight · {fmt(s.propellant / 1000, 1)} t of propellant left · {shipLive.replans} guidance plans
        </p>
      )}
      {scenario === "booster" && flight.outcome === "caught" && <p className={css.next}>The ship is still up there. Fly its return to the same tower, or end the mission here.</p>}
      <div>
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

/** The mission-control console around the 3D view. */
export function Console() {
  const { mode, error, view } = useShip();
  const [sheet, setSheet] = useState(false);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT")) return;
      const s = useShip.getState();
      if (e.code === "Space") {
        e.preventDefault();
        if (s.mode === "editing") go();
        else if (s.mode === "launch") s.beginFlight();
        else if (s.mode === "flying") s.setView({ paused: !s.view.paused });
        else s.reset();
      } else if (e.key === "r" || e.key === "R") s.reset();
      else if (e.key === "a" || e.key === "A") {
        if (s.mode === "flying" || s.mode === "done") s.setView({ analysis: !s.view.analysis, paused: !s.view.analysis && s.mode === "flying" ? true : s.view.paused });
      } else if (e.key >= "1" && e.key <= "6") s.setView({ camera: CAMERA_KEYS[Number(e.key) - 1] });
      else if (e.key === "," || e.key === ".") {
        const i = SPEEDS.indexOf(s.view.speed as (typeof SPEEDS)[number]) + (e.key === "," ? -1 : 1);
        if (i >= 0 && i < SPEEDS.length) s.setView({ speed: SPEEDS[i] });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const analysis = view.analysis && (mode === "flying" || mode === "done");
  return (
    <div className={css.hud} data-analysis={analysis || undefined} data-mode={mode}>
      <div className={css.frame} aria-hidden />
      <TopBar onSettings={() => setSheet((v) => !v)} />
      <aside className={css.left} data-sheet={sheet || undefined}>
        {analysis ? <AnalysisPanel /> : <Dock open={sheet} onClose={() => setSheet(false)} />}
      </aside>
      <aside className={css.right}>{mode === "editing" || mode === "launch" ? <Briefing /> : <Telemetry />}</aside>
      <PhaseBanner />
      <LaunchOverlay />
      <AnalysisIntro />
      <footer className={css.bottom}>
        <Caption />
        <ActionBar />
      </footer>
      <Verdict />
      {error && (
        <div className={css.error} role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
