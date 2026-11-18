"use client";

import { useFrame } from "@react-three/fiber";
import { Bloom, EffectComposer, SMAA, ToneMapping, Vignette } from "@react-three/postprocessing";
import { BlendFunction, Effect, ToneMappingMode } from "postprocessing";
import { useMemo, useRef } from "react";
import { Uniform } from "three";
import { analysisOn, useShip } from "@/lib/ship/store";
import { skin } from "./Blueprint";

export type Quality = "high" | "low";

const fragment = /* glsl */ `
  uniform float uProgress; // 0 = film look, 1 = the sweep has covered the picture
  uniform float uReal;     // 1 once the scene itself is the blueprint drawing
  uniform float uFlash;    // 1 at the swap, fading to 0
  uniform float uTime;

  float lum(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }

  vec3 draftingGrid(vec2 uv) {
    vec2 g = uv * vec2(aspect, 1.0);
    vec2 fine = abs(fract(g * 40.0) - 0.5);
    vec2 coarse = abs(fract(g * 8.0) - 0.5);
    float grid = smoothstep(0.47, 0.5, max(fine.x, fine.y)) * 0.045 + smoothstep(0.485, 0.5, max(coarse.x, coarse.y)) * 0.09;
    return grid * vec3(0.3, 0.7, 1.0);
  }

  void mainImage(const in vec4 inputColor, const in vec2 uv, out vec4 outputColor) {
    if (uProgress <= 0.0005) { outputColor = inputColor; return; }
    vec2 c = (uv - 0.5) * vec2(aspect, 1.0);
    float r = length(c);

    if (uReal > 0.5) {
      // The scene is the drawing now: add the paper (grid, a soft pool of
      // light, faint scan lines) and let the flash of the swap die away.
      vec3 col = inputColor.rgb;
      col += draftingGrid(uv) * (1.0 - 0.6 * smoothstep(0.0, 0.5, lum(col)));
      col *= 1.0 - 0.32 * smoothstep(0.35, 1.05, r);
      col *= 0.965 + 0.035 * sin(uv.y * resolution.y * 1.2 + uTime * 5.0);
      col += uFlash * vec3(0.45, 0.8, 1.0) * (0.5 + 0.5 * exp(-r * r * 3.0));
      outputColor = vec4(col, inputColor.a);
      return;
    }

    // The change spreads from the centre as a shockwave: a ring that bends
    // the film picture as it passes and leaves it blue behind it.
    float front = uProgress * 1.35;
    float d = r - front;
    float ripple = exp(-d * d * 260.0);
    vec2 dir = r > 1e-4 ? c / r : vec2(0.0);
    vec2 bent = uv - dir * ripple * 0.035 / vec2(aspect, 1.0);
    float twist = (1.0 - uProgress) * smoothstep(0.0, -0.35, d) * 0.12;
    vec2 q = (bent - 0.5) * vec2(aspect, 1.0);
    q = mat2(cos(twist), -sin(twist), sin(twist), cos(twist)) * q;
    bent = q / vec2(aspect, 1.0) + 0.5;
    vec4 src = texture2D(inputBuffer, bent);
    src.r = texture2D(inputBuffer, bent + dir * ripple * 0.012).r;
    src.b = texture2D(inputBuffer, bent - dir * ripple * 0.012).b;
    float revealed = smoothstep(0.012, -0.012, d);

    float l = lum(src.rgb);
    float lx = lum(texture2D(inputBuffer, bent + vec2(texelSize.x, 0.0)).rgb) - lum(texture2D(inputBuffer, bent - vec2(texelSize.x, 0.0)).rgb);
    float ly = lum(texture2D(inputBuffer, bent + vec2(0.0, texelSize.y)).rgb) - lum(texture2D(inputBuffer, bent - vec2(0.0, texelSize.y)).rgb);
    float edge = clamp(length(vec2(lx, ly)) * 9.0, 0.0, 1.0);

    // Behind the ring the world sinks into the paper's blue, its edges
    // glowing for a moment before the drawing replaces it.
    vec3 deep = vec3(0.006, 0.03, 0.095);
    vec3 mid = vec3(0.03, 0.2, 0.46);
    float sink = smoothstep(0.0, 0.5, -d);
    vec3 blue = mix(mix(deep, mid, smoothstep(0.0, 0.9, l)), deep, sink * 0.85);
    blue += edge * vec3(0.35, 0.8, 1.0) * (1.0 - sink * 0.7);
    blue += draftingGrid(uv);
    vec3 col = mix(src.rgb, blue, revealed);
    col += ripple * vec3(0.55, 0.95, 1.0) * 1.6;
    float d2 = r - front * 0.72;
    col += exp(-d2 * d2 * 900.0) * vec3(0.3, 0.7, 1.0) * 0.6;
    outputColor = vec4(col, inputColor.a);
  }
`;

/** The analysis view's transition and paper: a blue shockwave over the film picture, then grid and scan lines over the drawing. */
class BlueprintEffect extends Effect {
  constructor() {
    super("BlueprintEffect", fragment, {
      blendFunction: BlendFunction.NORMAL,
      uniforms: new Map<string, Uniform>([
        ["uProgress", new Uniform(0)],
        ["uReal", new Uniform(0)],
        ["uFlash", new Uniform(0)],
        ["uTime", new Uniform(0)],
      ]),
    });
  }
}

/** Seconds the blue shockwave takes to cross the picture. */
export const SWEEP = 0.95;

/**
 * The finish: glow on hot things, film tone mapping, vignette, and the
 * analysis view's transition. When the sweep has covered the picture the
 * scene itself is swapped for the blueprint drawing (`skin.on`), behind a
 * short flash; leaving the view swaps straight back.
 */
export function Post({ quality }: { quality: Quality }) {
  const blueprint = useMemo(() => new BlueprintEffect(), []);
  const progress = useRef(0);
  const flash = useRef(0);
  useFrame(({ clock }, dt) => {
    const want = analysisOn(useShip.getState());
    const step = Math.min(dt, 0.05);
    if (want) {
      progress.current = Math.min(1, progress.current + step / SWEEP);
      if (progress.current >= 1 && !skin.on) {
        skin.on = true;
        flash.current = 1;
      }
    } else {
      // (`SkinGuard` has already put the scene back.)
      progress.current = Math.max(0, progress.current - step * 3);
    }
    flash.current = Math.max(0, flash.current - step * 2.2);
    const p = progress.current;
    const u = blueprint.uniforms;
    (u.get("uProgress") as Uniform).value = want ? 1 - (1 - p) * (1 - p) : p * p;
    (u.get("uReal") as Uniform).value = skin.on ? 1 : 0;
    (u.get("uFlash") as Uniform).value = flash.current * flash.current;
    (u.get("uTime") as Uniform).value = clock.elapsedTime;
  });
  if (quality === "low") {
    return (
      <EffectComposer multisampling={0}>
        <ToneMapping mode={ToneMappingMode.ACES_FILMIC} />
        <primitive object={blueprint} dispose={null} />
        <SMAA />
      </EffectComposer>
    );
  }
  return (
    <EffectComposer multisampling={0}>
      <Bloom mipmapBlur luminanceThreshold={0.92} intensity={1.2} radius={0.72} />
      <ToneMapping mode={ToneMappingMode.ACES_FILMIC} />
      <primitive object={blueprint} dispose={null} />
      <Vignette offset={0.28} darkness={0.58} />
      <SMAA />
    </EffectComposer>
  );
}
