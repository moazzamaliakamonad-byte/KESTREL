"use client";

import dynamic from "next/dynamic";

// The scene and the solver only make sense in a browser: never pre-render them.
const App = dynamic(() => import("./App"), {
  ssr: false,
  loading: () => <div className="boot">Kestrel · loading the simulator</div>,
});

export function ClientRoot() {
  return <App />;
}
