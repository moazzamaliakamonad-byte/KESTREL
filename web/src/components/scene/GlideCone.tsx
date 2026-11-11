"use client";

import { useMemo } from "react";
import * as THREE from "three";

const RADIUS = 4500;

const vertex = /* glsl */ `
  varying vec3 vWorld;
  varying vec3 vNormal;
  void main() {
    vec4 w = modelMatrix * vec4(position, 1.0);
    vWorld = w.xyz;
    vNormal = normalize(mat3(modelMatrix) * normal);
    gl_Position = projectionMatrix * viewMatrix * w;
  }
`;

// Faint glowing funnel: brighter at grazing angles (fresnel), with radial and
// ring guide lines, fading out with distance from the pad.
const fragment = /* glsl */ `
  uniform vec3 uColor;
  uniform float uRadius;
  varying vec3 vWorld;
  varying vec3 vNormal;
  void main() {
    vec3 view = normalize(cameraPosition - vWorld);
    float fres = pow(1.0 - abs(dot(view, normalize(vNormal))), 2.0);
    float r = length(vWorld.xz);
    float ang = atan(vWorld.z, vWorld.x);
    float spokes = smoothstep(0.9985, 1.0, abs(cos(ang * 12.0)));
    float rings = smoothstep(0.994, 1.0, abs(cos(r * 0.012)));
    float fade = 1.0 - smoothstep(uRadius * 0.15, uRadius * 0.7, r);
    float a = (0.015 + 0.06 * fres + 0.1 * max(spokes, rings)) * fade;
    gl_FragColor = vec4(uColor * a, a);
  }
`;

/** The glide-slope cone: the vehicle must stay above this surface. */
export function GlideCone({ glideSlope }: { glideSlope: number }) {
  const height = Math.tan(glideSlope) * RADIUS;
  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: vertex,
        fragmentShader: fragment,
        uniforms: { uColor: { value: new THREE.Color("#7fe3ff") }, uRadius: { value: RADIUS } },
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
        side: THREE.DoubleSide,
      }),
    [],
  );
  if (!(height > 0)) return null;
  return (
    <mesh material={material} position-y={height / 2 + 0.5} renderOrder={2}>
      <cylinderGeometry args={[RADIUS, 0.01, height, 128, 1, true]} />
    </mesh>
  );
}
