import { describe, expect, it } from "vitest";
import { autoMode } from "./camera";

describe("automatic camera", () => {
  it("orbits while editing", () => {
    expect(autoMode("editing", false, false)).toBe("orbit");
  });

  it("chases during flight, then cuts to the pad near touchdown", () => {
    expect(autoMode("flying", false, false)).toBe("chase");
    expect(autoMode("flying", true, false)).toBe("pad");
  });

  it("orbits after touchdown", () => {
    expect(autoMode("touchdown", true, false)).toBe("orbit");
  });

  it("never switches automatically when reduced motion is requested", () => {
    expect(autoMode("flying", false, true)).toBe("orbit");
    expect(autoMode("flying", true, true)).toBe("orbit");
  });
});
