/**
 * The parts of each vehicle that the analysis view lets a visitor open,
 * and for each the calculation the simulator is doing there at this
 * instant: the model's own formula with the numbers from the current
 * snapshot in it. Nothing here is a separate estimate: the coefficients
 * come from Rust (`ship_scenario().model`), the state from the snapshot.
 * Pure functions, so they are tested without a browser.
 */

import { rotateInv, type Vec3 } from "./frames";
import { gateAt } from "./record";
import type { PlanDraw, ScenarioInfo, ShipSnapshot, Vehicle } from "./types";

export interface PartDef {
  id: string;
  name: string;
  /** Short label for the marker on the vehicle. */
  short: string;
  /** Where the marker sits and the camera looks: model axes (x belly, y nose, z −body y), m from the centre of mass. */
  at: Vec3;
  /** Camera distance when the part is open, m. */
  view: number;
}

/** One line of a part's sheet: a formula from the model and its value now. */
export interface Calc {
  label: string;
  /** The model's formula, in plain text. */
  formula?: string;
  value: string;
  /** 0–1 against a limit, drawn as a bar. */
  share?: number;
  /** The limit the share is against, as text. */
  limit?: string;
}

export interface PartSheet {
  title: string;
  /** What the part does, in a sentence. */
  blurb: string;
  calcs: Calc[];
}

const G0 = 9.80665;
const R = 4.5;

const SHIP_PARTS: PartDef[] = [
  { id: "avionics", name: "Nose · guidance and attitude", short: "Guidance", at: [0, 23, 0], view: 34 },
  { id: "flap0", name: "Forward flap · left", short: "Fwd flap L", at: [-1.3, 20.5, -(3.6 + 2.2)], view: 26 },
  { id: "flap1", name: "Forward flap · right", short: "Fwd flap R", at: [-1.3, 20.5, 3.6 + 2.2], view: 26 },
  { id: "shield", name: "Heat shield · airflow", short: "Heat shield", at: [R, 2, 0], view: 46 },
  { id: "tanks", name: "Propellant tanks · mass", short: "Tanks", at: [-R, -6, 0], view: 46 },
  { id: "pins", name: "Catch pins", short: "Catch pin", at: [0, 16, R + 0.9], view: 24 },
  { id: "rcs", name: "Cold-gas thrusters", short: "Thrusters", at: [-(R + 0.4), 12, 0], view: 24 },
  { id: "flap2", name: "Aft flap · left", short: "Aft flap L", at: [-0.6, -17.2, -(R + 2.8)], view: 30 },
  { id: "flap3", name: "Aft flap · right", short: "Aft flap R", at: [-0.6, -17.2, R + 2.8], view: 30 },
  { id: "engines", name: "Engines · thrust and gimbal", short: "Engines", at: [0, -26, 0], view: 26 },
];

const BOOSTER_PARTS: PartDef[] = [
  { id: "avionics", name: "Forward section · guidance and attitude", short: "Guidance", at: [0, 47, 0], view: 34 },
  { id: "fin0", name: "Grid fin · +y", short: "Grid fin +y", at: [0, 38, -(R + 3.6)], view: 24 },
  { id: "fin1", name: "Grid fin · −y", short: "Grid fin −y", at: [0, 38, R + 3.6], view: 24 },
  { id: "fin2", name: "Grid fin · +x", short: "Grid fin +x", at: [R + 3.6, 38, 0], view: 24 },
  { id: "pins", name: "Catch hardpoints", short: "Catch point", at: [0, 36, R + 1], view: 24 },
  { id: "rcs", name: "Cold-gas thrusters", short: "Thrusters", at: [-(R + 0.6), 40, 0], view: 24 },
  { id: "tanks", name: "Propellant tanks · mass", short: "Tanks", at: [-R, 6, 0], view: 60 },
  { id: "shield", name: "Hull and chines · airflow", short: "Airflow", at: [R * 0.75, -16, R * 0.75], view: 46 },
  { id: "engines", name: "Engines · thrust and gimbal", short: "Engines", at: [0, -28, 0], view: 28 },
];

export function partsOf(vehicle: Vehicle): PartDef[] {
  return vehicle === "booster" ? BOOSTER_PARTS : SHIP_PARTS;
}

export function partOf(vehicle: Vehicle, id: string | null): PartDef | null {
  return partsOf(vehicle).find((p) => p.id === id) ?? null;
}

const n = (v: number, d = 0) => (Number.isFinite(v) ? v.toFixed(d) : "—");
/** Newtons as kN or MN. */
const force = (f: number) => (Math.abs(f) >= 1e6 ? `${n(f / 1e6, 2)} MN` : `${n(f / 1e3, Math.abs(f) < 1e4 ? 2 : 1)} kN`);
const deg = (rad: number) => (rad * 180) / Math.PI;
const share = (v: number, limit: number) => Math.max(0, Math.min(1, Math.abs(v) / limit));

/** The air's velocity past the vehicle in body axes: axial part and normal part, m/s. */
export function airflow(s: ShipSnapshot): { axial: number; normal: number; speed: number; alphaDeg: number } {
  const w: Vec3 = [s.velocity[0] - s.wind[0], s.velocity[1] - s.wind[1], s.velocity[2] - s.wind[2]];
  const b = rotateInv(s.quat, w);
  const normal = Math.hypot(b[0], b[1]);
  return { axial: b[2], normal, speed: Math.hypot(...w), alphaDeg: deg(Math.atan2(normal, Math.abs(b[2]))) };
}

/** Height of the catch pins above the arms, m (the pins are on the body axis' height `pin_height`). */
export function pinsAboveArms(s: ShipSnapshot, info: ScenarioInfo): number {
  const up = Math.cos((s.tilt_deg * Math.PI) / 180);
  return s.position[2] + (info.ship.pin_height - info.ship.cm_height) * up - (info.catch?.arm_z ?? 0);
}

function engines(s: ShipSnapshot, info: ScenarioInfo): PartSheet {
  const lit = s.engines.filter((e) => e === "running").length;
  const failed = s.engines.filter((e) => e === "failed").length;
  const isp = info.tuning.isp;
  const ve = isp * G0;
  const mdot = s.thrust_mag / ve;
  const per = lit ? s.thrust_mag / lit : 0;
  const side = s.thrust_mag * Math.sin((s.gimbal_deg * Math.PI) / 180);
  const torque = side * info.ship.cm_height;
  const J = info.model.inertia_per_kg[0] * s.mass;
  return {
    title: "Engines",
    blurb:
      s.vehicle === "booster"
        ? "Thirteen landing engines brake the fall, then the centre three fly it onto the arms. They swivel together to steer."
        : "Three sea-level engines swing the ship upright; one flies it onto the arms. They swivel together to steer.",
    calcs: [
      { label: "Running", value: `${lit} of ${s.engines.length}${failed ? ` · ${failed} failed` : ""}` },
      { label: "Thrust", formula: "T = n · throttle · T_max", value: force(s.thrust_mag), share: lit ? share(s.thrust_mag, s.thrust_limits[1]) : 0, limit: lit ? `${force(s.thrust_limits[0])} to ${force(s.thrust_limits[1])}` : "engines off" },
      { label: "Each engine", formula: "T / n", value: lit ? `${force(per)} · ${n((per / info.ship.engine_max) * 100)} % throttle` : "—" },
      { label: "Propellant flow", formula: "ṁ = T / (I_sp · g₀)", value: `${n(mdot)} kg/s` },
      { label: "Exhaust speed", formula: "v_e = I_sp · g₀", value: `${n(ve)} m/s · I_sp ${n(isp)} s` },
      { label: "Gimbal angle", formula: "δ = angle(T, body axis)", value: `${n(s.gimbal_deg, 2)}°`, share: share(s.gimbal_deg, info.tuning.gimbal_max_deg), limit: `±${n(info.tuning.gimbal_max_deg)}°` },
      { label: "Steering force", formula: "F_side = T · sin δ", value: force(side) },
      { label: "Steering torque", formula: "τ = l_cm · T · sin δ", value: `${n(torque / 1e6, 2)} MN·m · arm ${n(info.ship.cm_height, 1)} m` },
      { label: "Turn acceleration", formula: "α = τ / J", value: `${n(deg(torque / J), 2)} °/s²` },
      { label: "Thrust to weight", formula: "T / (m · g)", value: n(s.thrust_mag / Math.max(1, s.weight), 2) },
    ],
  };
}

function tanks(s: ShipSnapshot, info: ScenarioInfo): PartSheet {
  const ve = info.tuning.isp * G0;
  const mdot = s.thrust_mag / ve;
  const dv = ve * Math.log(Math.max(1, s.mass / s.dry_mass));
  return {
    title: "Propellant and mass",
    blurb: "Liquid oxygen and methane. The vehicle gets lighter every second it burns, so the same thrust slows it harder.",
    calcs: [
      { label: "Propellant", value: `${n(s.propellant / 1000, 1)} t`, share: share(s.propellant, info.propellant), limit: `${n(info.propellant / 1000)} t at the start` },
      { label: "Mass", formula: "m = m_dry + m_prop", value: `${n(s.mass / 1000, 1)} t · dry ${n(s.dry_mass / 1000)} t` },
      { label: "Burn rate", formula: "ṁ = −T / (I_sp · g₀)", value: `−${n(mdot)} kg/s` },
      { label: "Time to empty", formula: "m_prop / ṁ", value: mdot > 1 ? `${n(s.propellant / mdot, 1)} s at this thrust` : "engines off" },
      { label: "Speed change left", formula: "Δv = v_e · ln(m / m_dry)", value: `${n(dv)} m/s` },
      { label: "Weight", formula: "W = m · g", value: force(s.weight) },
      { label: "Acceleration from thrust", formula: "T / m", value: `${n(s.accel_g, 2)} g` },
    ],
  };
}

function shield(s: ShipSnapshot, info: ScenarioInfo): PartSheet {
  const a = airflow(s);
  const m = info.model;
  const sa = Math.PI * info.ship.radius ** 2;
  const sn = info.ship.length * 2 * info.ship.radius;
  const axial = 0.5 * s.density * m.c_a * sa * a.axial * a.axial;
  const normal = 0.5 * s.density * (m.c_n * sn * a.normal + m.c_n_alpha * sa * Math.abs(a.axial)) * a.normal;
  return {
    title: s.vehicle === "booster" ? "Hull · airflow" : "Heat shield · airflow",
    blurb:
      s.vehicle === "booster"
        ? "The booster falls engines first. The air along the hull slows it; air across the hull, and the chines, give sideways lift."
        : "Belly first, the whole windward side is the brake. The tiles take the heating that the speed turns into.",
    calcs: [
      { label: "Air speed", formula: "w = v − wind", value: `${n(a.speed)} m/s · Mach ${n(s.mach, 2)}` },
      { label: "Angle to the airflow", formula: "α = atan(w_n / |w_a|)", value: `${n(a.alphaDeg, 1)}°` },
      { label: "Air density", formula: "ρ = ρ₀ · e^(−z / H)", value: `${s.density < 0.01 ? s.density.toExponential(2) : n(s.density, 3)} kg/m³` },
      { label: "Dynamic pressure", formula: "q = ½ ρ w²", value: `${n(s.q_dyn / 1000, 2)} kPa` },
      { label: "Drag across the hull", formula: "F_n = ½ ρ (C_n S_n w_n + C_nα S_a |w_a|) w_n", value: force(normal) },
      { label: "Drag along the hull", formula: "F_a = ½ ρ C_a S_a w_a²", value: force(axial) },
      { label: "Total air force", formula: "body + control surfaces", value: `${force(Math.hypot(...s.aero_force))} · ${n(Math.hypot(...s.aero_force) / Math.max(1, s.weight), 2)} × weight` },
      { label: "Heating at the nose", formula: "q̇ = k · √(ρ / R_n) · w³", value: `${n(s.heat_flux / 1000, s.heat_flux < 1e4 ? 2 : 0)} kW/m² (estimate, not in the dynamics)` },
    ],
  };
}

function flap(i: number, s: ShipSnapshot, info: ScenarioInfo): PartSheet {
  const a = airflow(s);
  const fore = i < 2;
  const area = fore ? info.model.fore_area : info.model.aft_area;
  const station = fore ? info.ship.fore_station : info.ship.aft_station;
  const e = s.flaps[i];
  const f = 0.5 * s.density * info.model.c_flap * area * e * a.normal * a.normal;
  const full = 0.5 * s.density * info.model.c_flap * area * a.normal * a.normal;
  return {
    title: `${fore ? "Forward" : "Aft"} flap · ${i % 2 === 0 ? "left" : "right"}`,
    blurb: "A flap is an air brake on a hinge. Folding it in sheds drag at its end of the ship: that is how the belly flop is steered.",
    calcs: [
      { label: "Exposure", formula: "e ∈ [0, 1]", value: `${n(e * 100)} %`, share: e, limit: "folded to fully out" },
      { label: "Air across the hull", formula: "w_n", value: `${n(a.normal, 1)} m/s` },
      { label: "Drag on this flap", formula: "F = ½ ρ C_f A e w_n²", value: `${force(f)} of ${force(full)} available` },
      { label: "Area · coefficient", value: `${n(area, 1)} m² · C_f ${n(info.model.c_flap, 2)}` },
      { label: "Lever arm", formula: "station from the centre of mass", value: `${n(station, 1)} m along · ${n(info.ship.flap_y, 1)} m out` },
      { label: "Pitch torque", formula: "τ = station × F", value: `${n((f * station) / 1e6, 2)} MN·m` },
      { label: "Roll torque", formula: "τ = y × F", value: `${n(((i % 2 === 0 ? 1 : -1) * f * info.ship.flap_y) / 1e6, 2)} MN·m` },
    ],
  };
}

function fin(i: number, s: ShipSnapshot, info: ScenarioInfo): PartSheet {
  const a = airflow(s);
  const m = info.model;
  const k = 0.5 * s.density * a.axial * a.axial * (m.fin_area ?? 0) * (m.fin_c_delta ?? 0);
  const d = s.flaps[i];
  const max = ((info.ship.fin_max_deg ?? 20) * Math.PI) / 180;
  const f = k * d;
  return {
    title: `Grid fin · ${["+y", "−y", "+x"][i]}`,
    blurb: "A lattice wing that turns on its shaft. The air rushing past the hull pushes on it sideways; three of them steer the fall.",
    calcs: [
      { label: "Deflection", formula: "δ", value: `${n(deg(d), 1)}°`, share: share(d, max), limit: `±${n(deg(max))}°` },
      { label: "Air along the hull", formula: "w_a", value: `${n(Math.abs(a.axial), 1)} m/s` },
      { label: "Force per radian", formula: "k = ½ ρ w_a² A C_δ", value: `${force(k)} / rad` },
      { label: "Force now", formula: "F = k · δ", value: force(f) },
      { label: "Most it can push", formula: "k · δ_max", value: force(k * max) },
      { label: "Steering torque", formula: "τ = station × F", value: `${n((f * (info.ship.fin_station ?? 0)) / 1e6, 2)} MN·m · arm ${n(info.ship.fin_station ?? 0, 1)} m` },
      { label: "Roll torque", formula: "τ = r × F", value: `${n((f * (m.fin_radius ?? 0)) / 1e3, 1)} kN·m` },
    ],
  };
}

function pins(s: ShipSnapshot, info: ScenarioInfo): PartSheet {
  const above = pinsAboveArms(s, info);
  const off = Math.hypot(s.position[0], s.position[1]);
  const side = Math.hypot(s.velocity[0], s.velocity[1]);
  const rate = deg(Math.hypot(...s.omega));
  const l = info.limits;
  const reach = info.catch?.capture_radius ?? 4;
  return {
    title: s.vehicle === "booster" ? "Catch hardpoints" : "Catch pins",
    blurb: "Two stubs near the top carry the whole vehicle. The tower's arms close under them and it is lowered the last metre onto their rails.",
    calcs: [
      { label: "Above the arms", formula: "z_pins − z_arms", value: `${n(above, 1)} m` },
      { label: "From the catch axis", formula: "√(x² + y²)", value: `${n(off, 2)} m`, share: share(off, reach), limit: `${n(reach, 1)} m of rail` },
      { label: "Sink rate", formula: "−v_z", value: `${n(-s.velocity[2], 2)} m/s`, share: share(Math.max(0, -s.velocity[2]), l.catch_down), limit: `${n(l.catch_down, 1)} m/s at contact` },
      { label: "Sideways speed", formula: "√(v_x² + v_y²)", value: `${n(side, 2)} m/s`, share: share(side, l.catch_side), limit: `${n(l.catch_side, 1)} m/s at contact` },
      { label: "Tilt", formula: "angle(body axis, vertical)", value: `${n(s.tilt_deg, 2)}°`, share: share(s.tilt_deg, l.catch_tilt_deg), limit: `${n(l.catch_tilt_deg)}° at contact` },
      { label: "Turn rate", formula: "|ω|", value: `${n(rate, 2)} °/s`, share: share(rate, l.catch_rate_deg), limit: `${n(l.catch_rate_deg)} °/s at contact` },
      { label: "Arms", value: s.arms ? "closing" : above > l.arms_close_above ? `open · close ${n(l.arms_close_above)} m above` : "open" },
    ],
  };
}

function rcs(s: ShipSnapshot, info: ScenarioInfo): PartSheet {
  const m = info.model;
  const firing = s.rcs.filter(Boolean).length;
  const arm = m.rcs_fore - m.rcs_aft;
  const J = m.inertia_per_kg[0] * s.mass;
  return {
    title: "Cold-gas thrusters",
    blurb: "Small jets of cold gas near both ends. They turn the vehicle when the engines are off and the air is too thin to steer with.",
    calcs: [
      { label: "Firing", value: `${firing} of ${s.rcs.length}` },
      { label: "Thrust of one", value: force(m.rcs_thrust) },
      { label: "Pitch torque of a pair", formula: "τ = F · (z_fore − z_aft)", value: `${n((m.rcs_thrust * arm) / 1e6, 2)} MN·m · arm ${n(arm, 1)} m` },
      { label: "Turn acceleration", formula: "α = τ / J", value: `${n(deg((m.rcs_thrust * arm) / J), 2)} °/s²` },
      { label: "Turn rate now", formula: "|ω|", value: `${n(deg(Math.hypot(...s.omega)), 2)} °/s` },
    ],
  };
}

function avionics(s: ShipSnapshot, info: ScenarioInfo, plan: PlanDraw | null, solveMs: number | undefined, replans: number): PartSheet {
  const fx = s.thrust_inertial[0] + s.aero_force[0];
  const fy = s.thrust_inertial[1] + s.aero_force[1];
  const fz = s.thrust_inertial[2] + s.aero_force[2] - s.weight;
  const acc = Math.hypot(fx, fy, fz) / s.mass;
  const g = gateAt(info.gate, s.bearing ?? 0);
  const gate = Math.hypot(s.position[0] - g[0], s.position[1] - g[1], s.position[2] - g[2]);
  const left = plan ? Math.max(0, plan.t0 + plan.final_time - s.t) : null;
  return {
    title: "Guidance and attitude",
    blurb: "The flight computer. Twice a second it solves for the cheapest burn that reaches the gate above the arms, then steers along it.",
    calcs: [
      { label: "Net acceleration", formula: "v̇ = (q T q* + F_aero) / m − g ẑ", value: `${n(acc, 1)} m/s² · ${n(acc / G0, 2)} g` },
      { label: "Attitude", formula: "q̇ = ½ q ⊗ ω", value: `q = [${s.quat.map((c) => n(c, 2)).join(", ")}]` },
      { label: "Tilt · turn rate", formula: "θ = angle(body axis, vertical)", value: `${n(s.tilt_deg, 1)}° · ${n(deg(Math.hypot(...s.omega)), 1)} °/s` },
      { label: "Rotation", formula: "J ω̇ = r_T × T + τ_aero − ω × Jω", value: `J = ${n((info.model.inertia_per_kg[0] * s.mass) / 1e6, 1)} × 10⁶ kg·m²` },
      { label: "Burn plan", formula: "min −m(t_f)  s.t.  x_k+1 = A_k x_k + B_k u_k,  ‖T‖ ≤ T_max", value: plan ? `${plan.position.length} nodes · ${n(plan.final_time, 1)} s long` : s.guide === "brake" ? "none: braking" : "none yet" },
      { label: "Plan time left", value: left !== null ? `${n(left, 1)} s` : "—" },
      { label: "Distance to the gate", formula: "‖r − r_gate‖", value: `${n(gate)} m` },
      { label: "Solver", value: solveMs !== undefined ? `${n(solveMs)} ms last · ${replans} plans this flight` : "no solve yet" },
      { label: "Burn starts at", formula: "height where the engines can still stop it", value: `${n(s.ignition_z)} m` },
    ],
  };
}

/** The sheet for part `id` at this instant. Unknown ids give the guidance sheet. */
export function partSheet(
  id: string,
  s: ShipSnapshot,
  info: ScenarioInfo,
  extra: { plan: PlanDraw | null; solveMs?: number; replans: number } = { plan: null, replans: 0 },
): PartSheet {
  if (id === "engines") return engines(s, info);
  if (id === "tanks") return tanks(s, info);
  if (id === "shield") return shield(s, info);
  if (id === "pins") return pins(s, info);
  if (id === "rcs") return rcs(s, info);
  if (id.startsWith("flap")) return flap(Number(id.slice(4)), s, info);
  if (id.startsWith("fin")) return fin(Number(id.slice(3)), s, info);
  return avionics(s, info, extra.plan, extra.solveMs, extra.replans);
}
