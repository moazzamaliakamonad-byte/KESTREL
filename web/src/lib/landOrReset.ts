import { canLand } from "./state";
import { useApp } from "./store";

/** Land when a plan for the current start is ready; reset after a flight. */
export function landOrReset() {
  const s = useApp.getState();
  if (canLand(s)) s.dispatch({ type: "land" });
  else if (s.phase === "flying" || s.phase === "touchdown") s.dispatch({ type: "reset" });
}
