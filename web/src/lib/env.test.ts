import { describe, expect, it } from "vitest";
import { BASE_PATH } from "./env";

describe("env", () => {
  it("base path is empty when NEXT_PUBLIC_BASE_PATH is unset", () => {
    expect(process.env.NEXT_PUBLIC_BASE_PATH).toBeUndefined();
    expect(BASE_PATH).toBe("");
  });
});
