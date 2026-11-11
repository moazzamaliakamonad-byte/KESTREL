/**
 * Procedural Mars-like terrain: fractal value noise plus seeded craters, made
 * flat around the landing pad. Physics coordinates (x east, y north, metres);
 * returns height above the pad in metres. Deterministic for a given seed.
 */

export const TERRAIN = {
  size: 20000,
  amplitude: 40,
  baseWavelength: 900,
  octaves: 5,
  craters: 12,
  padFlatRadius: 120,
  padBlendRadius: 300,
} as const;

/** Integer hash → [0, 1). */
function hash(ix: number, iy: number, seed: number): number {
  let h = (ix * 374761393 + iy * 668265263 + seed * 2147483647) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

const smooth = (t: number) => t * t * (3 - 2 * t);

/** Smoothly interpolated value noise in [−1, 1]. */
function valueNoise(x: number, y: number, seed: number): number {
  const ix = Math.floor(x);
  const iy = Math.floor(y);
  const fx = smooth(x - ix);
  const fy = smooth(y - iy);
  const a = hash(ix, iy, seed);
  const b = hash(ix + 1, iy, seed);
  const c = hash(ix, iy + 1, seed);
  const d = hash(ix + 1, iy + 1, seed);
  const top = a + (b - a) * fx;
  const bottom = c + (d - c) * fx;
  return (top + (bottom - top) * fy) * 2 - 1;
}

function fractal(x: number, y: number, seed: number): number {
  let sum = 0;
  let amp = 1;
  let norm = 0;
  let freq = 1 / TERRAIN.baseWavelength;
  for (let o = 0; o < TERRAIN.octaves; o++) {
    sum += amp * valueNoise(x * freq, y * freq, seed + o * 101);
    norm += amp;
    amp *= 0.5;
    freq *= 2;
  }
  return (sum / norm) * TERRAIN.amplitude;
}

interface Crater {
  x: number;
  y: number;
  r: number;
}

const craterCache = new Map<number, Crater[]>();

function craters(seed: number): Crater[] {
  const cached = craterCache.get(seed);
  if (cached) return cached;
  const list: Crater[] = [];
  for (let i = 0; list.length < TERRAIN.craters && i < 1000; i++) {
    const x = (hash(i, 1, seed) - 0.5) * 7000;
    const y = (hash(i, 2, seed) - 0.5) * 7000;
    const r = 40 + hash(i, 3, seed) * 180;
    if (Math.hypot(x, y) - r > TERRAIN.padBlendRadius + 100) list.push({ x, y, r });
  }
  craterCache.set(seed, list);
  return list;
}

/** Bowl below ground level with a raised rim, as a function of distance. */
function craterProfile(dist: number, r: number): number {
  const d = dist / r;
  const bowl = d < 1 ? r * 0.12 * (d * d - 1) : 0;
  const rim = r * 0.05 * Math.exp(-(((d - 1) / 0.25) ** 2));
  return bowl + rim;
}

export function heightAt(x: number, y: number, seed: number): number {
  let h = fractal(x, y, seed);
  for (const c of craters(seed)) {
    const dist = Math.hypot(x - c.x, y - c.y);
    if (dist < c.r * 2) h += craterProfile(dist, c.r);
  }
  const r = Math.hypot(x, y);
  const t = Math.min(
    Math.max((r - TERRAIN.padFlatRadius) / (TERRAIN.padBlendRadius - TERRAIN.padFlatRadius), 0),
    1,
  );
  return h * smooth(t);
}
