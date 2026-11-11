"use client";

import { Booster } from "./scene/Booster";
import { DemoLoop } from "./scene/DemoLoop";
import { GlideCone } from "./scene/GlideCone";
import { Pad } from "./scene/Pad";
import { Stage } from "./scene/Stage";
import { Terrain } from "./scene/Terrain";

const DEFAULT_GLIDE_SLOPE = (4 * Math.PI) / 180;

/** Client root: the 3D scene (and, later, the solver and HUD). */
export default function KestrelApp() {
  return (
    <div className="app">
      <Stage>
        <Terrain />
        <Pad blinking={false} />
        <GlideCone glideSlope={DEFAULT_GLIDE_SLOPE} />
        <Booster />
        <DemoLoop />
      </Stage>
    </div>
  );
}
