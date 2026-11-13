"use client";

import { Component, type ReactNode } from "react";
import { useApp } from "@/lib/store";
import s from "./hud.module.css";

/** A full-page explanation with a reload button. */
export function FallbackCard({ title, message }: { title: string; message: string }) {
  return (
    <div className={s.fallback} role="alert">
      <div className={`${s.fallbackCard} ${s.glass}`}>
        <h2>{title}</h2>
        <p>{message}</p>
        <button className={s.land} onClick={() => window.location.reload()}>
          Reload
        </button>
      </div>
    </div>
  );
}

/** Shown when the solver can't run at all. */
export function Fallback() {
  const phase = useApp((st) => st.phase);
  const failure = useApp((st) => st.failure);
  if (phase !== "failed") return null;
  return <FallbackCard title="The guidance solver couldn't start" message={failure ?? ""} />;
}

/**
 * Catches errors while creating the 3D view (e.g. no WebGL) so the page
 * explains the problem instead of going blank.
 */
export class SceneErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <FallbackCard
          title="The 3D view couldn't start"
          message={`This page needs WebGL (hardware graphics in the browser). ${this.state.error.message}`}
        />
      );
    }
    return this.props.children;
  }
}
