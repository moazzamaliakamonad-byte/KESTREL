"use client";

import { Line } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useMemo, useState } from "react";
import { toScene, type Vec3 } from "@/lib/ship/frames";
import { shipLive } from "@/lib/ship/live";
import { useShip } from "@/lib/ship/store";

const scene = (pts: Vec3[]) => pts.map((p) => toScene(p));

/** Preview (where this start leads), the current SCvx plan, and the flown trail. */
export function ShipPaths() {
  const preview = useShip((s) => s.preview);
  const mode = useShip((s) => s.mode);
  const [plan, setPlan] = useState<Vec3[] | null>(null);
  const [trail, setTrail] = useState<Vec3[]>([]);
  const [version, setVersion] = useState(-1);
  const [trailLen, setTrailLen] = useState(0);

  useFrame(() => {
    if (shipLive.planVersion !== version) {
      setVersion(shipLive.planVersion);
      setPlan(shipLive.plan ? scene(shipLive.plan.position) : null);
    }
    // Redraw the trail every few points (not every frame).
    if (shipLive.trail.length >= trailLen + 4 || shipLive.trail.length < trailLen) {
      setTrailLen(shipLive.trail.length);
      setTrail(scene(shipLive.trail));
    }
  });

  const previewPts = useMemo(() => (preview?.ok && preview.path.length > 1 ? scene(preview.path) : null), [preview]);

  return (
    <group>
      {previewPts && mode === "editing" && (
        <Line points={previewPts} color="#7fe3ff" lineWidth={2} dashed dashSize={40} gapSize={25} transparent opacity={0.8} />
      )}
      {plan && plan.length > 1 && mode !== "editing" && <Line points={plan} color="#ffffff" lineWidth={2.5} transparent opacity={0.85} />}
      {trail.length > 1 && mode !== "editing" && <Line points={trail} color="#ffb35c" lineWidth={2} transparent opacity={0.7} />}
    </group>
  );
}
