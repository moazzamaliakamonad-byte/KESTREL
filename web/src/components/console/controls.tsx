"use client";

import { useEffect, useState, type ReactNode } from "react";
import css from "./console.module.css";

/** Re-render every `ms` (for numbers read from `shipLive`). */
export function useTick(ms: number) {
  const [, set] = useState(0);
  useEffect(() => {
    const id = setInterval(() => set((n) => n + 1), ms);
    return () => clearInterval(id);
  }, [ms]);
}

/** `performance.now()`, refreshed every `ms` (reading the clock during render is not allowed). */
export function useNow(ms: number): number {
  const [now, set] = useState(0);
  useEffect(() => {
    const id = setInterval(() => set(performance.now()), ms);
    return () => clearInterval(id);
  }, [ms]);
  return now;
}

export function Seg<T extends string | number>({
  value,
  options,
  onChange,
  label,
  disabled,
}: {
  value: T;
  options: [T, string, boolean?][];
  onChange: (v: T) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <div className={css.seg} role="group" aria-label={label}>
      {options.map(([v, text, off]) => (
        <button key={String(v)} aria-pressed={v === value} disabled={disabled || off} onClick={() => onChange(v)}>
          {text}
        </button>
      ))}
    </div>
  );
}

export function Slider({
  label,
  value,
  range,
  step,
  onChange,
  format,
  tag,
  disabled,
}: {
  label: string;
  value: number;
  range: [number, number];
  step: number;
  onChange: (v: number) => void;
  format: (v: number) => string;
  /** "pub" = a published figure, "ill" = illustrative (not published). */
  tag?: "pub" | "ill";
  disabled?: boolean;
}) {
  return (
    <label className={css.slider} data-disabled={disabled || undefined}>
      <span>
        <em>
          {label}
          {tag && <i data-tag={tag}>{tag === "pub" ? "published" : "illustrative"}</i>}
        </em>
        <b>{format(value)}</b>
      </span>
      <input type="range" min={range[0]} max={range[1]} step={step} value={value} disabled={disabled} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}

export function Toggle({ label, value, onChange }: { label: string; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <button className={css.toggle} role="switch" aria-checked={value} onClick={() => onChange(!value)}>
      <span>{label}</span>
      <i />
    </button>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={css.field}>
      <span className={css.label}>{label}</span>
      {children}
    </div>
  );
}

export function Panel({ title, children, aside, className }: { title?: string; children: ReactNode; aside?: ReactNode; className?: string }) {
  return (
    <section className={`${css.panel} ${className ?? ""}`}>
      {title && (
        <header className={css.panelHead}>
          <h2>{title}</h2>
          {aside}
        </header>
      )}
      {children}
    </section>
  );
}

export const fmt = (v: number, d = 0) => (Number.isFinite(v) ? v.toFixed(d) : "—");
