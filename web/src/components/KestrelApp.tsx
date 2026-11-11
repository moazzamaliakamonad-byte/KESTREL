"use client";

import { OrbitControls } from "@react-three/drei";
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
        <OrbitControls target={[800, 400, 0]} maxDistance={9000} minDistance={20} />
      </Stage>
    </div>
  );
}
