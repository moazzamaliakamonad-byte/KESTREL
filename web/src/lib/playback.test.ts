import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/example-response.json";
import { sampleAt, toScene } from "./playback";
import type { PlanResponse, Trajectory, Vec3, Vehicle } from "./types";

const response = fixture as PlanResponse;
if (!response.ok) throw new Error("fixture must be a successful plan");
const traj: Trajectory = response.trajectory;
const vehicle: Vehicle = response.vehicle;

const norm = (v: Vec3) => Math.hypot(v[0], v[1], v[2]);
const close = (a: number, b: number) =>
  Math.abs(a - b) <= Math.max(1e-6, 1e-6 * Math.max(Math.abs(a), Math.abs(b)));

describe("sampleAt", () => {
  it("reproduces every node of the solved trajectory", () => {
    traj.t.forEach((tk, k) => {
      const s = sampleAt(traj, vehicle, tk);
      for (let i = 0; i < 3; i++) {
        expect(close(s.position[i], traj.position[k][i]), `pos node ${k}[${i}]`).toBe(true);
        expect(close(s.velocity[i], traj.velocity[k][i]), `vel node ${k}[${i}]`).toBe(true);
      }
      expect(close(s.mass, traj.mass[k]), `mass node ${k}`).toBe(true);
    });
  });

  it("integrating across each interval arrives at the next node", () => {
    // Sampling exactly at t[k+1] would just read node k+1; sample a hair
    // earlier so the constant-acceleration formulas of interval k are used.
    for (let k = 0; k + 1 < traj.t.length; k++) {
      const s = sampleAt(traj, vehicle, traj.t[k + 1] - 1e-9);
      for (let i = 0; i < 3; i++) {
        const dp = Math.abs(s.position[i] - traj.position[k + 1][i]);
        const dv = Math.abs(s.velocity[i] - traj.velocity[k + 1][i]);
        expect(dp, `pos into node ${k + 1}[${i}]`).toBeLessThan(1e-4);
        expect(dv, `vel into node ${k + 1}[${i}]`).toBeLessThan(1e-4);
      }
      expect(Math.abs(s.mass - traj.mass[k + 1]), `mass into ${k + 1}`).toBeLessThan(1e-6);
    }
  });

  it("ends at rest on the pad", () => {
    const s = sampleAt(traj, vehicle, traj.final_time);
    expect(norm(s.position)).toBeLessThan(0.01);
    expect(norm(s.velocity)).toBeLessThan(0.01);
    expect(s.done).toBe(true);
  });

  it("mass never increases", () => {
    let last = Infinity;
    for (let i = 0; i <= 200; i++) {
      const m = sampleAt(traj, vehicle, (traj.final_time * i) / 200).mass;
      expect(m).toBeLessThanOrEqual(last + 1e-9);
      last = m;
    }
  });

  it("displayed thrust stays within the engine limits inside intervals", () => {
    for (let k = 0; k < traj.thrust.length; k++) {
      const mid = (traj.t[k] + traj.t[k + 1]) / 2;
      const mag = sampleAt(traj, vehicle, mid).thrustMag;
      expect(mag).toBeGreaterThanOrEqual(vehicle.min_thrust * 0.995);
      expect(mag).toBeLessThanOrEqual(vehicle.max_thrust * 1.001);
    }
  });

  it("clamps time outside [0, final_time] and NaN", () => {
    const start = sampleAt(traj, vehicle, 0);
    const end = sampleAt(traj, vehicle, traj.final_time);
    expect(sampleAt(traj, vehicle, -5)).toEqual(start);
    expect(sampleAt(traj, vehicle, Number.NaN)).toEqual(start);
    expect(sampleAt(traj, vehicle, traj.final_time + 100)).toEqual(end);
    expect(end.thrustMag).toBe(0);
  });
});

describe("toScene", () => {
  it("maps physics z-up to three.js y-up", () => {
    expect(toScene([1, 2, 3])).toEqual([1, 3, -2]);
  });
});
