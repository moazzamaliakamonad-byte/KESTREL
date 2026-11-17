"use client";

import { useEffect, useState } from "react";
import { CARD_HANDOVER, COUNT_FROM, IGNITION_BEFORE, LAUNCH, cardStart, launchCues, launchState, type LaunchState } from "@/lib/ship/launch";
import { shipLive } from "@/lib/ship/live";
import { commentator } from "@/lib/ship/runtime";
import { LINES, countLine } from "@/lib/ship/phrases";
import { PHASE_LABEL, phasesOf, useShip } from "@/lib/ship/store";
import type { ShipPhase } from "@/lib/ship/types";
import css from "./console.module.css";

/** What the title card says for the flight about to be simulated. */
function cardText(scenario: string, parked: boolean, altitude: number, orbit: { rangeKm: number } | null): { kicker: string; title: string; line: string } {
  if (orbit) return { kicker: "From orbit", title: "Ship entry and return", line: `120 km up at 7.5 km/s, ${orbit.rangeKm.toLocaleString("en-GB")} km from the tower` };
  const km = `${(altitude / 1000).toFixed(altitude < 10_000 ? 1 : 0)} km`;
  if (scenario === "booster") return { kicker: "After stage separation", title: "Booster return", line: `${km} above the launch site, falling engines first` };
  return { kicker: parked ? "The booster is down · next" : "After its flight", title: "Ship return", line: `${km} above the launch site, belly to the wind` };
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));
/** A cue this late (a stalled tab) is skipped rather than said out of place, s. */
const CUE_WINDOW = 1.2;

const cueText = (c: { line?: string; count?: number }) => (c.count ? countLine(c.count) : LINES[c.line as keyof typeof LINES]);

/** Every line of the opening sequence (to fetch their clips before it starts). */
export function launchLines(): string[] {
  return launchCues().map(cueText);
}

/**
 * The opening sequence's overlay and its clock: counts down from ten,
 * calls liftoff and stage separation, fades through black at each cut,
 * shows the title card, and starts the simulated flight behind the card so
 * the picture is already the return when the card clears. The number on
 * screen, the number spoken and the mission clock all come from one
 * `launchState`. Says plainly that the ascent is an illustration.
 * Skipping jumps straight to the card.
 */
export function LaunchOverlay() {
  const launchAt = useShip((s) => s.launchAt);
  const scenario = useShip((s) => s.scenario);
  const parked = useShip((s) => s.parked);
  const altitude = useShip((s) => s.start.position[2]);
  const orbit = useShip((s) => s.orbit);
  const [ls, setLs] = useState<LaunchState | null>(null);

  useEffect(() => {
    const first = useShip.getState();
    if (first.mode !== "launch") return;
    const withStack = first.intro === "stack";
    const handover = cardStart(withStack) + CARD_HANDOVER;
    const cues = withStack ? launchCues() : [];
    let next = 0;
    let offset = 0;
    let begun = false;
    const stop = () => {
      clearInterval(id);
      shipLive.hold = false;
      shipLive.launchClock = null;
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
      shipLive.launchClock = s.clock;
      // Say every cue whose moment has come (a skipped sequence says no more).
      while (!begun && next < cues.length && cues[next].at <= elapsed) {
        const cue = cues[next++];
        if (elapsed - cue.at > CUE_WINDOW) continue;
        commentator.enabled = st.view.voice;
        commentator.say([{ text: cueText(cue), urgent: cue.urgent, quiet: !!cue.count }]);
        shipLive.caption = commentator.caption;
      }
      if (s.phase === "card" && s.t >= CARD_HANDOVER && !begun) {
        begun = true;
        st.beginFlight();
      }
      // The simulation waits behind the card.
      shipLive.hold = begun;
    };
    const id = setInterval(tick, 33);
    tick();
    return stop;
  }, [launchAt]);

  if (!ls) return null;
  const card = cardText(scenario, parked, altitude, orbit);
  const staging = ls.phase === "staging";
  const cardOpacity = ls.phase === "card" ? Math.min(clamp01(ls.t / 1.2), clamp01((LAUNCH.card - ls.t) / 1.4)) : 0;
  const counting = ls.phase === "countdown" && ls.count > 0;
  const status = !counting
    ? "Full stack on the pad"
    : ls.count > 7
      ? "All systems are go"
      : ls.count > IGNITION_BEFORE
        ? "Flight computer has the count"
        : "Ignition sequence start";
  return (
    <div className={css.launch} data-phase={ls.phase}>
      <div className={css.fade} style={{ opacity: ls.phase === "card" ? clamp01((LAUNCH.card - ls.t) / 1.4) : ls.fade }} />
      {ls.phase === "countdown" && (
        <div className={css.count} data-on={counting || undefined}>
          <span>{counting ? "T minus" : "Stand by"}</span>
          {/* Re-keyed each second so the pulse restarts with the number. */}
          <b key={ls.count}>{counting ? ls.count : COUNT_FROM}</b>
          <em>{status}</em>
        </div>
      )}
      {ls.phase === "climb" && ls.t < 5 && (
        <div className={css.liftoff}>
          <b>Liftoff</b>
        </div>
      )}
      {ls.phase === "climb" && ls.t >= 6.5 && ls.t < LAUNCH.climb - 1.5 && (
        <div className={css.lower} data-long>
          <span>Ascent</span>
          <b>Thirty-three engines · the full stack</b>
        </div>
      )}
      {staging && ls.t > 1 && ls.t < LAUNCH.staging - 1.2 && (
        <div className={css.lower} key={ls.apart > 0 ? "apart" : "joined"} data-long>
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
  entry: "Belly first into the air at 7.5 km/s; guidance rolls the lift to steer",
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
  const phase = useShip((s) => (s.mode === "flying" && !s.view.analysis ? s.flight?.phase : undefined));
  const first = useShip((s) => phasesOf(s.scenario, s.orbit !== null)[0]);
  // The opening phase is announced by the title card.
  if (!phase || phase === "done" || phase === first) return null;
  return (
    <div className={css.banner} key={phase}>
      <span>Now</span>
      <b>{PHASE_LABEL[phase]}</b>
      <em>{PHASE_LINE[phase]}</em>
    </div>
  );
}
