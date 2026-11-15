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
