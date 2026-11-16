"use client";

import { useEffect, useState } from "react";
import { CARD_HANDOVER, LAUNCH, cardStart, launchState, type LaunchState } from "@/lib/ship/launch";
import { shipLive } from "@/lib/ship/live";
import { commentator } from "@/lib/ship/runtime";
import { LINES } from "@/lib/ship/phrases";
import { PHASE_LABEL, phasesOf, useShip } from "@/lib/ship/store";
import type { ShipPhase } from "@/lib/ship/types";
import css from "./console.module.css";

/** What the title card says for the flight about to be simulated. */
function cardText(scenario: string, parked: boolean, altitude: number): { kicker: string; title: string; line: string } {
  const km = `${(altitude / 1000).toFixed(altitude < 10_000 ? 1 : 0)} km`;
  if (scenario === "booster") return { kicker: "After stage separation", title: "Booster return", line: `${km} above the launch site, falling engines first` };
  if (scenario === "mars") return { kicker: "After the cruise to Mars", title: "Mars descent", line: `${km} above the surface, belly to the wind` };
  return { kicker: parked ? "The booster is down · next" : "After its flight", title: "Ship return", line: `${km} above the launch site, belly to the wind` };
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/**
 * The opening sequence's overlay and its clock: counts down, calls liftoff
 * and stage separation, fades through black at each cut, shows the title
 * card, and starts the simulated flight behind the card so the picture is
 * already the return when the card clears. Says plainly that the ascent is
 * an illustration. Skipping jumps straight to the card.
 */
export function LaunchOverlay() {
  const launchAt = useShip((s) => s.launchAt);
  const scenario = useShip((s) => s.scenario);
  const parked = useShip((s) => s.parked);
  const altitude = useShip((s) => s.start.position[2]);
  const [ls, setLs] = useState<LaunchState | null>(null);

  useEffect(() => {
    const first = useShip.getState();
    if (first.mode !== "launch") return;
    const withStack = first.intro === "stack";
    const handover = cardStart(withStack) + CARD_HANDOVER;
    let offset = 0;
    let begun = false;
    const said = new Set<string>();
    const once = (key: string, text: string, urgent = false) => {
      if (said.has(key)) return;
      said.add(key);
      commentator.enabled = useShip.getState().view.voice;
      commentator.say([{ text, urgent }]);
      shipLive.caption = commentator.caption;
    };
    const stop = () => {
      clearInterval(id);
      shipLive.hold = false;
      setLs(null);
    };
    const tick = () => {
      const st = useShip.getState();
      if (st.mode === "editing") return stop();
      let elapsed = (performance.now() - launchAt) / 1000 + offset;
      if (st.mode !== "launch" && !begun) {
        // Skipped: the flight has begun. Jump into the card to cover the cut.
        begun = true;
        if (elapsed < handover) {
          offset += handover - elapsed;
          elapsed = handover;
        }
      }
      const s = launchState(elapsed, withStack);
      if (s.phase === "over") return stop();
      setLs(s);
      if (withStack && !begun) {
        if (s.phase === "countdown") once("count", LINES.countdown);
        if (s.phase === "climb") once("liftoff", LINES.liftoff, true);
        if (s.phase === "climb" && s.t > 6) once("clear", LINES.cleared);
        if (s.phase === "staging" && s.t > 0.9 && s.apart === 0) once("throttle", LINES.throttling);
        if (s.phase === "staging" && s.apart > 0) once("staging", LINES.staging, true);
        if (s.phase === "staging" && s.apart > 3.4) once("home", LINES.home);
      }
      if (s.phase === "card" && s.t >= CARD_HANDOVER && !begun) {
        begun = true;
        st.beginFlight();
      }
      // The simulation waits behind the card.
      shipLive.hold = begun;
    };
    const id = setInterval(tick, 50);
    tick();
    return stop;
  }, [launchAt]);

  if (!ls) return null;
  const card = cardText(scenario, parked, altitude);
  const staging = ls.phase === "staging";
  const cardOpacity = ls.phase === "card" ? Math.min(clamp01(ls.t / 0.9), clamp01((LAUNCH.card - ls.t) / 1.1)) : 0;
  return (
    <div className={css.launch} data-phase={ls.phase}>
      <div className={css.fade} style={{ opacity: ls.phase === "card" ? clamp01((LAUNCH.card - ls.t) / 1.1) : ls.fade }} />
      {ls.phase === "countdown" && (
        <div className={css.count} key={ls.count}>
          <span>T minus</span>
          <b>{ls.count}</b>
        </div>
      )}
      {ls.phase === "climb" && ls.t < 4 && (
        <div className={css.liftoff}>
          <b>Liftoff</b>
        </div>
      )}
      {ls.phase === "climb" && ls.t >= 5 && ls.t < LAUNCH.climb - 1 && (
        <div className={css.lower}>
          <span>Ascent</span>
          <b>Thirty-three engines · the full stack</b>
        </div>
      )}
      {staging && ls.t > 0.8 && ls.t < LAUNCH.staging - 0.8 && (
        <div className={css.lower} key={ls.apart > 0 ? "apart" : "joined"}>
          <span>{ls.apart > 0 ? "Stage separation" : "Approaching staging · 64 km"}</span>
          <b>{ls.apart > 0 ? "Two vehicles from here" : "Booster throttles down, ship lights its engines"}</b>
        </div>
      )}
      {(ls.phase === "countdown" || ls.phase === "climb" || staging) && ls.fade < 0.5 && (
        <p className={css.disclaimer}>Ascent and staging: an illustration. The simulation starts with the return.</p>
      )}
      {ls.phase === "card" && (
        <div className={css.card} style={{ opacity: cardOpacity }}>
          <i />
          <span>{card.kicker}</span>
          <h1>{card.title}</h1>
          <p>{card.line}</p>
          <em>Simulation begins</em>
        </div>
      )}
    </div>
  );
}

const PHASE_LINE: Record<ShipPhase, string> = {
  descent: "Three grid fins steer it toward the tower",
  startup: "The inner thirteen engines light",
  "belly-flop": "Four flaps hold it belly to the wind",
  flip: "The engines light and swing it upright",
  "landing-burn": "Guidance re-plans the burn several times a second",
  terminal: "Sliding in over the catch arms",
  done: "",
};

/**
 * A lower-third that names each flight phase as it begins. It animates in
 * and out by itself (CSS), restarted by the phase changing.
 */
export function PhaseBanner() {
  const phase = useShip((s) => (s.mode === "flying" ? s.flight?.phase : undefined));
  const first = useShip((s) => phasesOf(s.scenario)[0]);
  const mars = useShip((s) => s.planet === "mars");
  // The opening phase is announced by the title card.
  if (!phase || phase === "done" || phase === first) return null;
  return (
    <div className={css.banner} key={phase}>
      <span>Now</span>
      <b>{PHASE_LABEL[phase]}</b>
      <em>{phase === "terminal" && mars ? "Legs down, settling onto the surface" : PHASE_LINE[phase]}</em>
    </div>
  );
}

/**
 * The mark drawing itself in as the analysis view takes over the picture.
 * Mounted while the view is on; its animation ends invisible.
 */
export function AnalysisIntro() {
  const on = useShip((s) => s.view.analysis && (s.mode === "flying" || s.mode === "done"));
  if (!on) return null;
  return (
    <div className={css.intro} aria-hidden>
      <svg viewBox="-100 -100 200 200" className={css.introMark}>
        <circle r={78} className={css.introRing} />
        <circle r={92} className={css.introRing2} />
        {Array.from({ length: 36 }, (_, i) => (
          <line key={i} x1={0} y1={-84} x2={0} y2={i % 3 === 0 ? -72 : -78} transform={`rotate(${i * 10})`} className={css.introTick} style={{ animationDelay: `${i * 12}ms` }} />
        ))}
        {/* The kestrel: a swept wing mark. */}
        <path d="M0 -46 L40 44 L0 20 L-40 44 Z" className={css.introPath} />
        <path d="M0 -46 L0 20" className={css.introPath} />
        <line x1={-100} x2={100} y1={0} y2={0} className={css.introCross} />
        <line y1={-100} y2={100} x1={0} x2={0} className={css.introCross} />
      </svg>
      <div className={css.introText}>
        <b>ANALYSIS</b>
        <span>Kestrel · flight dynamics · live</span>
      </div>
      <div className={css.introBars}>
        <i />
        <i />
        <i />
      </div>
    </div>
  );
}
