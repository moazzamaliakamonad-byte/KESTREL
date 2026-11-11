"use client";

import { useMemo } from "react";
import * as THREE from "three";
import { heightAt, TERRAIN } from "@/lib/terrain";

const SEED = 7;
const SEGMENTS = 400;

const DUST = new THREE.Color("#c4774a");
const RUST = new THREE.Color("#8e3f22");
const BASALT = new THREE.Color("#3b2420");
const HIGHLIGHT = new THREE.Color("#d9a07a");

/** 20 km of procedural Mars ground, coloured by height and slope. */
export function Terrain() {
  const geometry = useMemo(() => {
    const g = new THREE.PlaneGeometry(TERRAIN.size, TERRAIN.size, SEGMENTS, SEGMENTS);
    g.rotateX(-Math.PI / 2); // lie flat: plane x/z in scene space
    const pos = g.attributes.position as THREE.BufferAttribute;
    for (let i = 0; i < pos.count; i++) {
      // Scene (x, z) → physics (x, y = −z).
      pos.setY(i, heightAt(pos.getX(i), -pos.getZ(i), SEED));
    }
    g.computeVertexNormals();

    const normals = g.attributes.normal as THREE.BufferAttribute;
    const colors = new Float32Array(pos.count * 3);
    const c = new THREE.Color();
    for (let i = 0; i < pos.count; i++) {
      const h = pos.getY(i);
      const slope = 1 - normals.getY(i); // 0 flat … 1 vertical
      c.copy(DUST).lerp(RUST, THREE.MathUtils.clamp(0.5 - h / 60, 0, 1));
      c.lerp(BASALT, THREE.MathUtils.clamp(slope * 6, 0, 0.85));
      c.lerp(HIGHLIGHT, THREE.MathUtils.clamp((h - 20) / 40, 0, 0.35));
      // Fine speckle so flat ground isn't plastic-smooth.
      const n = Math.sin(pos.getX(i) * 0.37) * Math.cos(pos.getZ(i) * 0.41);
      c.multiplyScalar(0.94 + 0.06 * n);
      colors.set([c.r, c.g, c.b], i * 3);
    }
    g.setAttribute("color", new THREE.BufferAttribute(colors, 3));
    return g;
  }, []);

  return (
    <mesh geometry={geometry} receiveShadow>
      <meshStandardMaterial vertexColors roughness={0.96} metalness={0} />
    </mesh>
  );
}
