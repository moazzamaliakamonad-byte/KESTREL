/**
 * The state the page shows: the live one, or, while the analysis view is
 * replaying a recorded instant (`view.scrubT`), that instant's frame.
 */

import { shipLive } from "./live";
import { frameAt } from "./record";
import { useShip } from "./store";
import type { ShipSnapshot } from "./types";

export function shown(): ShipSnapshot | null {
  const t = useShip.getState().view.scrubT;
  if (t === null) return shipLive.snapshot;
  return frameAt(shipLive.frames, t) ?? shipLive.snapshot;
}
