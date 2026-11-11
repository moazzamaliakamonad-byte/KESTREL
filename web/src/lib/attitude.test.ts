import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { smoothQuaternion, targetQuaternion, type Quat } from "./attitude";
import { toScene } from "./playback";

const IDENTITY: Quat = [0, 0, 0, 1];
const upAfter = (q: Quat) =>
  new THREE.Vector3(0, 1, 0).applyQuaternion(new THREE.Quaternion(...q));

describe("attitude", () => {
  it("vertical thrust needs no rotation", () => {
    const q = targetQuaternion([0, 0, 5000], IDENTITY);
    expect(upAfter(q).distanceTo(new THREE.Vector3(0, 1, 0))).toBeLessThan(1e-9);
  });

  it("tilted thrust rotates the body axis onto the thrust direction", () => {
    const q = targetQuaternion([1000, 0, 1000], IDENTITY);
    const want = new THREE.Vector3(...toScene([1, 0, 1])).normalize();
    expect(upAfter(q).distanceTo(want)).toBeLessThan(1e-9);
  });

  it("keeps the previous attitude when there is no thrust", () => {
    const prev: Quat = [0.1, 0.2, 0.3, Math.sqrt(1 - 0.14)];
    expect(targetQuaternion([0, 0, 0.5], prev)).toEqual(prev);
  });

  it("smoothing converges to the target", () => {
    const target = targetQuaternion([3000, 2000, 4000], IDENTITY);
    let q = IDENTITY;
    for (let i = 0; i < 120; i++) q = smoothQuaternion(q, target, 1 / 60);
    const angle = new THREE.Quaternion(...q).angleTo(new THREE.Quaternion(...target));
    expect(THREE.MathUtils.radToDeg(angle)).toBeLessThan(0.5);
  });
});
