"use client";

import { useMemo } from "react";
import * as THREE from "three";

/**
 * Sun direction in scene space (y up): azimuth 160°, elevation 8°. Placed in
 * front of the default camera so the blue sunset glow is in view and the
 * booster is dramatically back-lit.
 */
export const SUN_DIRECTION = (() => {
  const az = THREE.MathUtils.degToRad(160);
  const el = THREE.MathUtils.degToRad(8);
  return new THREE.Vector3(Math.cos(el) * Math.sin(az), Math.sin(el), Math.cos(el) * Math.cos(az));
})();

export const HORIZON_COLOR = new THREE.Color("#b98d6c");

const vertex = /* glsl */ `
  varying vec3 vDir;
  void main() {
    vDir = normalize((modelMatrix * vec4(position, 1.0)).xyz - cameraPosition);
    gl_Position = projectionMatrix * viewMatrix * modelMatrix * vec4(position, 1.0);
    gl_Position.z = gl_Position.w; // always at the far plane
  }
`;

// Mars dusk: butterscotch-brown sky, with the famous blue glow around the low
// sun (fine dust scatters blue light forward), a hot sun disc and faint stars.
const fragment = /* glsl */ `
  uniform vec3 uSun;
  uniform vec3 uHorizon;
  varying vec3 vDir;

  float hash(vec3 p) { return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453); }

  void main() {
    vec3 d = normalize(vDir);
    float elev = clamp(d.y, -0.2, 1.0);
    vec3 zenith = vec3(0.13, 0.08, 0.07);
    vec3 col = mix(uHorizon, zenith, smoothstep(0.0, 0.65, elev));
    col = mix(col, uHorizon * 0.55, smoothstep(0.0, -0.2, elev));

    float c = max(dot(d, normalize(uSun)), 0.0);
    vec3 blue = vec3(0.42, 0.62, 0.86);
    col = mix(col, blue, pow(c, 24.0) * 0.85);
    col += blue * pow(c, 6.0) * 0.18;
    col += vec3(1.6, 1.45, 1.25) * smoothstep(0.99975, 0.99995, c) * 1.6;

    vec3 q = floor(d * 420.0);
    float star = step(0.9975, hash(q)) * smoothstep(0.35, 0.85, elev);
    col += vec3(star) * 0.55;

    gl_FragColor = vec4(col, 1.0);
    #include <colorspace_fragment>
  }
`;

export function Sky() {
  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: vertex,
        fragmentShader: fragment,
        uniforms: {
          uSun: { value: SUN_DIRECTION.clone() },
          uHorizon: { value: HORIZON_COLOR.clone() },
        },
        side: THREE.BackSide,
        depthWrite: false,
        fog: false,
      }),
    [],
  );
  return (
    <mesh material={material} renderOrder={-1} frustumCulled={false}>
      <sphereGeometry args={[15000, 48, 24]} />
    </mesh>
  );
}
