import { describe, expect, it } from "vitest";
import { autoMode } from "./camera";

describe("automatic camera", () => {
  it("orbits while editing", () => {
    expect(autoMode("editing", 0, 36, false)).toBe("orbit");
  });

  it("chases during flight, then cuts to the pad for the last 8 s", () => {
    expect(autoMode("flying", 5, 36, false)).toBe("chase");
    expect(autoMode("flying", 27.9, 36, false)).toBe("chase");
    expect(autoMode("flying", 28.1, 36, false)).toBe("pad");
  });

  it("orbits after touchdown", () => {
    expect(autoMode("touchdown", 36, 36, false)).toBe("orbit");
  });

  it("never switches automatically when reduced motion is requested", () => {
    expect(autoMode("flying", 5, 36, true)).toBe("orbit");
    expect(autoMode("flying", 30, 36, true)).toBe("orbit");
  });
});
