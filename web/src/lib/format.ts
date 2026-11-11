const DASH = "—";

export function fmtNumber(v: number, digits: number): string {
  return Number.isFinite(v) ? v.toFixed(digits) : DASH;
}

const withUnit = (v: number, digits: number, unit: string) =>
  Number.isFinite(v) ? `${v.toFixed(digits)} ${unit}` : DASH;

export const fmtTime = (s: number) => (Number.isFinite(s) ? `T+${s.toFixed(1)} s` : DASH);
export const fmtMass = (kg: number) => withUnit(kg, 1, "kg");
export const fmtForce = (n: number) => withUnit(n / 1000, 2, "kN");
export const fmtSpeed = (mps: number) => withUnit(mps, 1, "m/s");
export const fmtAltitude = (m: number) => withUnit(m, 0, "m");
