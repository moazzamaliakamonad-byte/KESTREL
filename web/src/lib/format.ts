const DASH = "—";

/** Fixed digits, without "-0.0" for values that round to zero. */
function fixed(v: number, digits: number): string {
  const text = v.toFixed(digits);
  return Number(text) === 0 ? (0).toFixed(digits) : text;
}

export function fmtNumber(v: number, digits: number): string {
  return Number.isFinite(v) ? fixed(v, digits) : DASH;
}

const withUnit = (v: number, digits: number, unit: string) =>
  Number.isFinite(v) ? `${fixed(v, digits)} ${unit}` : DASH;

export const fmtTime = (s: number) => (Number.isFinite(s) ? `T+${fixed(s, 1)} s` : DASH);
export const fmtMass = (kg: number) => withUnit(kg, 1, "kg");
export const fmtForce = (n: number) => withUnit(n / 1000, 2, "kN");
export const fmtSpeed = (mps: number) => withUnit(mps, 1, "m/s");
export const fmtAltitude = (m: number) => withUnit(m, 0, "m");
