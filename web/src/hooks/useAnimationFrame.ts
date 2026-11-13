"use client";

import { useEffect, useRef } from "react";

/** Run `fn` every animation frame while mounted (latest `fn` is used). */
export function useAnimationFrame(fn: (now: number) => void) {
  const latest = useRef(fn);
  useEffect(() => {
    latest.current = fn;
  });
  useEffect(() => {
    let id = 0;
    const loop = (now: number) => {
      latest.current(now);
      id = requestAnimationFrame(loop);
    };
    id = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(id);
  }, []);
}
