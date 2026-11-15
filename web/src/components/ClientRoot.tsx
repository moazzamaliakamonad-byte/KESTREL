"use client";

import dynamic from "next/dynamic";

// The scene and the solver only make sense in a browser: never pre-render them.
const ShipApp = dynamic(() => import("./ship/ShipApp"), {
  ssr: false,
  loading: () => <div className="boot">Loading Kestrel…</div>,
});

export function ClientRoot() {
  return <ShipApp />;
}
