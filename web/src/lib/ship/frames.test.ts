import { describe, expect, it } from "vitest";
import { quatToScene, rotate, rotateScene, toScene, type PQuat, type Vec3 } from "./frames";

const close = (a: Vec3, b: Vec3) => a.forEach((c, i) => expect(c).toBeCloseTo(b[i], 9));

describe("ship frames", () => {
  it("upright is the model's rest pose", () => {
    expect(quatToScene([1, 0, 0, 0])).toEqual([0, 0, -0, 1]);
  });

  it("the model's nose follows the physics body axis", () => {
    // Belly down, nose toward −x (Rust ship_control::belly_attitude(0)):
    // +90° about y, then 180° about z.
    const h = Math.SQRT1_2;
    const about = (ax: Vec3, a: number): PQuat => [Math.cos(a / 2), ...(ax.map((c) => c * Math.sin(a / 2)) as Vec3)];
    const mul = (a: PQuat, b: PQuat): PQuat => [
      a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
      a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
      a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
      a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ];
    const q = mul(about([0, 0, 1], Math.PI), about([0, 1, 0], Math.PI / 2));
    close(rotate(q, [0, 0, 1]), [-1, 0, 0]);
    // Model nose (+Y) and belly (+X) in the scene.
    const s = quatToScene(q);
    close(rotateScene(s, [0, 1, 0]), toScene(rotate(q, [0, 0, 1])));
    close(rotateScene(s, [1, 0, 0]), toScene(rotate(q, [1, 0, 0])));
    close(rotateScene(s, [0, 0, -1]), toScene(rotate(q, [0, 1, 0])));
    expect(h).toBeGreaterThan(0);
  });
});

import { quatFromZ, rotateInv, startAttitude } from "./frames";

describe("start attitudes", () => {
  it("quatFromZ turns +z onto the direction", () => {
    for (const d of [[0, 0, 1], [1, 0, 0], [0.3, -0.4, 0.5], [0, 0, -1]] as Vec3[]) {
      const n = Math.hypot(...d);
      const out = rotate(quatFromZ(d), [0, 0, 1]);
      out.forEach((c, i) => expect(c).toBeCloseTo(d[i] / n, 9));
    }
  });

  it("the booster starts engines first: its axis opposes the velocity", () => {
    const v: Vec3 = [-30, 10, -420];
    const axis = rotate(startAttitude("booster", v), [0, 0, 1]);
    const n = Math.hypot(...v);
    axis.forEach((c, i) => expect(c).toBeCloseTo(-v[i] / n, 9));
  });

  it("the ship starts belly down with its nose toward −x", () => {
    const q = startAttitude("ship", [0, 0, -80]);
    expect(rotate(q, [0, 0, 1])[0]).toBeCloseTo(-1);
    expect(rotate(q, [1, 0, 0])[2]).toBeCloseTo(-1);
    expect(rotateInv(q, rotate(q, [1, 2, 3]))[1]).toBeCloseTo(2);
  });
});
