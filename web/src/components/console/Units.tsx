"use client";

import { useEffect, useState } from "react";
import { SOURCES } from "@/lib/sources";
import { UNITS, splitUnits } from "@/lib/units";
import css from "./console.module.css";

/** Formatted text with each unit marked, so pointing at it explains it. */
export function Units({ text }: { text: string }) {
  return (
    <>
      {splitUnits(text).map((p, i) =>
        p.unit ? (
          <span key={i} className={css.unit} data-unit={p.unit}>
            {p.text}
          </span>
        ) : (
          p.text
        ),
      )}
    </>
  );
}

/**
 * One tooltip for every unit on the page: it follows the pointer over any
 * element marked `data-unit` (a single element, so scrolling panels never
 * clip it).
 */
export function UnitTip() {
  const [tip, setTip] = useState<{ head: string; sub: string; body: string; x: number; y: number } | null>(null);
  useEffect(() => {
    const move = (e: PointerEvent) => {
      const el = (e.target as Element | null)?.closest?.("[data-unit], [data-note]");
      const unit = el?.getAttribute("data-unit");
      const note = el?.getAttribute("data-note");
      if (unit && UNITS[unit]) setTip({ head: unit, sub: UNITS[unit].name, body: UNITS[unit].what, x: e.clientX, y: e.clientY });
      else if (note && SOURCES[note]) setTip({ head: SOURCES[note].title, sub: "", body: SOURCES[note].text, x: e.clientX, y: e.clientY });
      else setTip(null);
    };
    window.addEventListener("pointermove", move);
    return () => window.removeEventListener("pointermove", move);
  }, []);
  if (!tip) return null;
  // Keep it on screen: flip to the left near the right edge.
  const left = tip.x > window.innerWidth - 300;
  return (
    <div className={css.unitTip} style={{ top: tip.y + 16, left: left ? undefined : tip.x + 14, right: left ? window.innerWidth - tip.x + 14 : undefined }} role="tooltip">
      <b>
        {tip.head} <span>{tip.sub}</span>
      </b>
      <p>{tip.body}</p>
    </div>
  );
}
