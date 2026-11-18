"use client";

import { useFrame, type ThreeEvent } from "@react-three/fiber";
import { useEffect, useMemo, useRef, type ReactNode, type RefObject } from "react";
import * as THREE from "three";
import { analysisOn, useShip } from "@/lib/ship/store";

/**
 * The analysis view's drawing of the vehicle: every solid part becomes a
 * translucent blue shell with its station rings, stringers and hard edges
 * drawn as bright lines (a blueprint), and each named part can be pointed
 * at and clicked to open its calculations. Everything else in the scene
 * steps aside while this is on.
 *
 * `skin.on` is the one switch the scene reads each frame: it turns true
 * when the blue transition has swept the picture (`Post`), not at the
 * click, so the film picture is never seen snapping to lines.
 */
export const skin: { on: boolean; hover: string | null } = { on: false, hover: null };

/** A named, clickable part of a vehicle model (see `lib/ship/parts.ts` for the names). */
export function Part({ id, children }: { id: string; children: ReactNode }) {
  const over = (e: ThreeEvent<PointerEvent>, id_: string | null) => {
    if (!skin.on) return;
    e.stopPropagation();
    skin.hover = id_;
    document.body.style.cursor = id_ ? "pointer" : "";
  };
  return (
    <group
      userData={{ part: id }}
      onClick={(e) => {
        if (!skin.on) return;
        e.stopPropagation();
        const st = useShip.getState();
        st.setView({ part: st.view.part === id ? null : id });
      }}
      onPointerOver={(e) => over(e, id)}
      onPointerOut={(e) => over(e, null)}
    >
      {children}
    </group>
  );
}

const FILL = new THREE.Color("#0a3a78");
const LINE = new THREE.Color("#9fe9ff");
const HOT = new THREE.Color("#ffd27a");

function shellMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    transparent: true,
    side: THREE.DoubleSide,
    fog: false,
    uniforms: { uFill: { value: FILL }, uLine: { value: LINE }, uHot: { value: HOT }, uSel: { value: 0 }, uHover: { value: 0 }, uTime: { value: 0 } },
    vertexShader: /* glsl */ `
      #include <common>
      #include <logdepthbuf_pars_vertex>
      varying vec3 vN; varying vec3 vV; varying vec3 vP;
      void main() {
        vP = position;
        vN = normalize(normalMatrix * normal);
        vec4 mv = modelViewMatrix * vec4(position, 1.0);
        vV = normalize(-mv.xyz);
        gl_Position = projectionMatrix * mv;
        #include <logdepthbuf_vertex>
      }`,
    fragmentShader: /* glsl */ `
      #include <logdepthbuf_pars_fragment>
      uniform vec3 uFill; uniform vec3 uLine; uniform vec3 uHot;
      uniform float uSel; uniform float uHover; uniform float uTime;
      varying vec3 vN; varying vec3 vV; varying vec3 vP;
      float gridLine(float x) {
        return 1.0 - smoothstep(0.0, 1.6 * fwidth(x), abs(fract(x - 0.5) - 0.5));
      }
      void main() {
        #include <logdepthbuf_fragment>
        // Bright where the surface turns away: the outline of the shape.
        float rim = pow(1.0 - abs(dot(normalize(vN), normalize(vV))), 2.2);
        // Station rings every 3 m along the part and stringers every 15° around it.
        float rings = gridLine(vP.y / 3.0);
        float around = atan(vP.z, vP.x) / 6.2831853 * 24.0;
        // (No stringer where the angle wraps: its derivative is not defined there.)
        float stringers = gridLine(around) * step(fwidth(around), 0.5);
        float lines = max(rings, stringers);
        float pick = max(uSel, uHover * 0.6);
        vec3 line = mix(uLine, uHot, pick);
        float pulse = 0.85 + 0.15 * sin(uTime * 4.0);
        vec3 col = uFill * (0.55 + 0.9 * pick) + line * (rim * 1.1 + lines * 0.5 * (1.0 + pick * pulse));
        float alpha = 0.22 + 0.2 * pick + rim * 0.7 + lines * 0.5;
        gl_FragColor = vec4(col, clamp(alpha, 0.0, 0.96));
      }`,
  });
}

interface Swapped {
  mesh: THREE.Mesh;
  original: THREE.Material | THREE.Material[];
  edges: THREE.LineSegments;
}

function partOfObject(o: THREE.Object3D): string {
  for (let p: THREE.Object3D | null = o; p; p = p.parent) {
    if (typeof p.userData.part === "string") return p.userData.part;
  }
  return "";
}

interface SkinState {
  shown: boolean;
  swapped: Swapped[];
  materials: Map<string, THREE.ShaderMaterial>;
  edge: THREE.LineBasicMaterial;
  hot: THREE.LineBasicMaterial;
}

function newSkinState(): SkinState {
  return {
    shown: false,
    swapped: [],
    materials: new Map(),
    edge: new THREE.LineBasicMaterial({ color: "#c9f6ff", transparent: true, opacity: 0.85, fog: false, toneMapped: false }),
    hot: new THREE.LineBasicMaterial({ color: "#ffe2a8", transparent: true, opacity: 1, fog: false, toneMapped: false }),
  };
}

function restore(st: SkinState) {
  for (const s of st.swapped) {
    s.mesh.material = s.original;
    s.edges.visible = false;
  }
  st.swapped = [];
  st.shown = false;
}

function apply(st: SkinState, g: THREE.Group) {
  st.shown = true;
  g.traverse((o) => {
    const mesh = o as THREE.Mesh;
    if (!mesh.isMesh) return;
    const m = mesh.material as THREE.Material;
    if (Array.isArray(mesh.material) || !(m as THREE.MeshStandardMaterial).isMeshStandardMaterial || (m.transparent && !m.alphaTest)) return;
    const id = partOfObject(mesh);
    let shell = st.materials.get(id);
    if (!shell) {
      shell = shellMaterial();
      st.materials.set(id, shell);
    }
    let edges = mesh.userData.edges as THREE.LineSegments | undefined;
    if (!edges) {
      edges = new THREE.LineSegments(new THREE.EdgesGeometry(mesh.geometry, 28), st.edge);
      edges.raycast = () => {};
      mesh.userData.edges = edges;
      mesh.add(edges);
    }
    edges.visible = true;
    st.swapped.push({ mesh, original: mesh.material, edges });
    mesh.material = shell;
  });
}

function update(st: SkinState, time: number, selected: string | null) {
  for (const [id, m] of st.materials) {
    const u = m.uniforms;
    u.uTime.value = time;
    u.uSel.value += ((id && id === selected ? 1 : 0) - u.uSel.value) * 0.2;
    u.uHover.value += ((id && id === skin.hover ? 1 : 0) - u.uHover.value) * 0.3;
  }
  for (const s of st.swapped) {
    const id = partOfObject(s.mesh);
    s.edges.material = id && (id === selected || id === skin.hover) ? st.hot : st.edge;
  }
}

function dispose(st: SkinState) {
  restore(st);
  for (const m of st.materials.values()) m.dispose();
  st.edge.dispose();
  st.hot.dispose();
}

/**
 * Swaps the solid meshes under `root` to the blueprint look while
 * `skin.on`, and back. Effects (plumes, heating, puffs: anything that is
 * not an opaque standard material) hide themselves by reading `skin.on`.
 */
export function BlueprintSkin({ root }: { root: RefObject<THREE.Group | null> }) {
  const state = useRef<SkinState | null>(null);
  useEffect(() => {
    const st = newSkinState();
    state.current = st;
    return () => dispose(st);
  }, []);
  useFrame(({ clock }) => {
    const g = root.current;
    const st = state.current;
    if (!g || !st) return;
    if (skin.on && !st.shown) apply(st, g);
    else if (!skin.on && st.shown) restore(st);
    if (st.shown) update(st, clock.elapsedTime, useShip.getState().view.part);
  });
  return null;
}

/** The blue behind the drawing: a soft pool of light on deep blue, wherever the camera is. */
export function Backdrop() {
  const mesh = useRef<THREE.Mesh>(null);
  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        side: THREE.BackSide,
        depthWrite: false,
        depthTest: false,
        fog: false,
        vertexShader: /* glsl */ `varying vec3 vDir; void main() { vDir = normalize(position); gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
        fragmentShader: /* glsl */ `
          varying vec3 vDir;
          void main() {
            vec3 d = normalize(vDir);
            vec3 deep = vec3(0.004, 0.022, 0.075);
            vec3 mid = vec3(0.012, 0.075, 0.2);
            float h = smoothstep(-0.7, 0.9, d.y);
            gl_FragColor = vec4(mix(deep, mid, h * 0.8), 1.0);
          }`,
      }),
    [],
  );
  useFrame(({ camera }) => {
    const m = mesh.current;
    if (!m) return;
    m.visible = skin.on;
    m.position.copy(camera.position);
  });
  return (
    <mesh ref={mesh} material={material} renderOrder={-9} frustumCulled={false} visible={false}>
      <sphereGeometry args={[900, 24, 12]} />
    </mesh>
  );
}

/** Keeps `skin` in step when the analysis view is left (the entry is timed by `Post`). */
export function SkinGuard() {
  useFrame(() => {
    if (skin.on && !analysisOn(useShip.getState())) {
      skin.on = false;
      skin.hover = null;
      document.body.style.cursor = "";
    }
  });
  return null;
}
