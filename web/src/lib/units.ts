/**
 * Every unit the page shows, what it is, and a sense of scale, for the
 * tooltip shown when a visitor points at one. `splitUnits` finds them in
 * a formatted value ("1528 kg/s · Mach 0.28") so any text can carry them.
 */

export interface UnitInfo {
  name: string;
  what: string;
}

export const UNITS: Record<string, UnitInfo> = {
  m: { name: "metre", what: "Distance. The ship is 52 m tall, about a 17-storey building." },
  km: { name: "kilometre", what: "1 000 metres." },
  "m/s": { name: "metres per second", what: "Speed. 1 m/s is 3.6 km/h; a fast walk is about 2 m/s." },
  "km/h": { name: "kilometres per hour", what: "Speed, as on a car's speedometer. 3.6 km/h = 1 m/s." },
  "m/s²": { name: "metres per second squared", what: "Acceleration: how much the speed changes each second. Gravity is 9.81 m/s²." },
  g: { name: "g (standard gravity)", what: "Acceleration as a multiple of Earth's gravity, 9.81 m/s². 1 g is what you feel standing still." },
  Mach: { name: "Mach number", what: "Speed divided by the speed of sound in the air around the vehicle (about 340 m/s at sea level). Above Mach 1 is supersonic." },
  kPa: { name: "kilopascal", what: "Pressure: 1 000 newtons on every square metre. Here, dynamic pressure, how hard the air pushes on the vehicle (½ρv²). Sea-level air pressure is 101 kPa." },
  N: { name: "newton", what: "Force. About the weight of an apple." },
  kN: { name: "kilonewton", what: "Force, 1 000 newtons: about the weight of 100 kg." },
  MN: { name: "meganewton", what: "Force, one million newtons: about the weight of 100 tonnes. One Raptor engine gives about 2.5 MN." },
  "MN·m": { name: "meganewton-metre", what: "Torque, the turning effect of a force: force times its distance from the centre of mass." },
  "kN·m": { name: "kilonewton-metre", what: "Torque, 1 000 newton-metres." },
  t: { name: "tonne", what: "Mass, 1 000 kg." },
  kg: { name: "kilogram", what: "Mass: how much matter there is (what the engines must push)." },
  "kg/s": { name: "kilograms per second", what: "How fast propellant is burned: mass flow." },
  "kg/m³": { name: "kilograms per cubic metre", what: "Air density. 1.225 kg/m³ at sea level, falling by a factor of e (about 2.7) every 8.5 km of height in this model." },
  "kW/m²": { name: "kilowatts per square metre", what: "Heating rate on the surface. Sunlight at noon is about 1 kW/m²." },
  s: { name: "second", what: "Time. Simulated time: at 2× speed one second passes in half a real one." },
  ms: { name: "millisecond", what: "A thousandth of a second. Here, how long the computer took to plan." },
  "°": { name: "degree", what: "Angle. 90° is a right angle." },
  "°/s": { name: "degrees per second", what: "How fast the vehicle is turning." },
  "°/s²": { name: "degrees per second squared", what: "How fast the turning rate is changing (angular acceleration)." },
  "%": { name: "percent", what: "A share of the whole: throttle as a share of full thrust, or a flap's exposure." },
  "/ rad": { name: "per radian", what: "Per radian of deflection. A radian is about 57.3°." },
};

/** Longest first, so "m/s²" wins over "m/s" and "m". */
const KEYS = Object.keys(UNITS).sort((a, b) => b.length - a.length);
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
// A unit stands after a number (and a space) or alone, and is followed by
// the end, a space or punctuation, never by more letters.
const PATTERN = new RegExp(`(?<=\\d\\s?|^|\\s)(${KEYS.map(escape).join("|")})(?=$|[\\s·,;:)\\]])`, "g");

export type Piece = { text: string; unit?: string };

/** Split formatted text into plain pieces and known units. */
export function splitUnits(text: string): Piece[] {
  const out: Piece[] = [];
  let last = 0;
  for (const m of text.matchAll(PATTERN)) {
    const at = m.index ?? 0;
    // "Mach" only names the unit when a number follows or precedes it.
    if (at > last) out.push({ text: text.slice(last, at) });
    out.push({ text: m[0], unit: m[0] });
    last = at + m[0].length;
  }
  if (last < text.length) out.push({ text: text.slice(last) });
  return out;
}
