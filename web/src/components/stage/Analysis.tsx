"use client";

import { shown } from "@/lib/ship/shown";
import { Html } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { quatToScene, rotate, toScene, type Vec3 } from "@/lib/ship/frames";
import { shipLive } from "@/lib/ship/live";
import { partsOf } from "@/lib/ship/parts";
import { analysisOn, useShip } from "@/lib/ship/store";
import { skin } from "./Blueprint";

/**
 * The analysis view's drawing around the blueprint: the vectors in the
 * equations of motion (velocity, thrust, aerodynamic force, weight), the
 * body axis against the vertical, the engine gimbal cone, the plan's
 * nodes, the gate it is flying to and the ignition altitude; and a marker
 * on every part that can be opened. Every arrow is the simulator's number
 * at this instant, to scale with the others of its kind. With a part open
 * the vectors step back so the part can be read.
 */

const Y = new THREE.Vector3(0, 1, 0);

interface Arrow {
  group: THREE.Group;
  shaft: THREE.Mesh;
  head: THREE.Mesh;
}

function makeArrow(color: string): Arrow {
  const material = new THREE.MeshBasicMaterial({ color, depthTest: false, transparent: true, opacity: 0.95, fog: false, toneMapped: false });
  const shaft = new THREE.Mesh(new THREE.CylinderGeometry(1, 1, 1, 10).translate(0, 0.5, 0), material);
  const head = new THREE.Mesh(new THREE.ConeGeometry(2.6, 6, 14).translate(0, 3, 0), material);
  const group = new THREE.Group();
  group.add(shaft, head);
  group.renderOrder = 30;
  shaft.renderOrder = 30;
  head.renderOrder = 30;
  // Arrows are read, not clicked: clicks go through to the part behind.
  shaft.raycast = () => {};
  head.raycast = () => {};
  return { group, shaft, head };
}

const dir = new THREE.Vector3();

/** Point an arrow from `origin` along scene vector `vec`, `length` metres long, `w` thick. */
function setArrow(a: Arrow, origin: THREE.Vector3, vec: Vec3, length: number, w: number, show: boolean) {
  dir.set(vec[0], vec[1], vec[2]);
  const n = dir.length();
  a.group.visible = show && n > 1e-6 && length > 6 * w;
  if (!a.group.visible) return;
  a.group.position.copy(origin);
  a.group.quaternion.setFromUnitVectors(Y, dir.divideScalar(n));
  const headLen = 6 * w;
  a.shaft.scale.set(w, Math.max(0.01, length - headLen), w);
  a.head.scale.setScalar(w);
  a.head.position.y = length - headLen;
}

const fmt = (x: number, d = 0) => x.toFixed(d);
const VECTORS = ["velocity", "thrust", "aero", "weight"] as const;
const noRay = () => {};

export function Analysis() {
  const on = useShip(analysisOn);
  const vehicle = useShip((s) => s.vehicle);
  const selected = useShip((s) => s.view.part);
  const setView = useShip((s) => s.setView);
  // The drawing appears when the blue sweep has covered the picture.
  const [ready, setReady] = useState(false);
  const parts = useMemo(() => partsOf(vehicle), [vehicle]);
  const arrows = useMemo(
    () => ({
      velocity: makeArrow("#7ff0ff"),
      thrust: makeArrow("#ff9a2e"),
      aero: makeArrow("#ff4fc8"),
      weight: makeArrow("#ffffff"),
    }),
    [],
  );
  const root = useRef<THREE.Group>(null);
  const axis = useRef<THREE.Mesh>(null);
  const plumb = useRef<THREE.Mesh>(null);
  const cone = useRef<THREE.Mesh>(null);
  const gate = useRef<THREE.Group>(null);
  const ignition = useRef<THREE.Mesh>(null);
  const nodes = useRef<THREE.InstancedMesh>(null);
  const labels = useRef<Record<string, { g: THREE.Group | null; el: HTMLDivElement | null }>>({
    velocity: { g: null, el: null },
    thrust: { g: null, el: null },
    aero: { g: null, el: null },
    weight: { g: null, el: null },
    tilt: { g: null, el: null },
    gate: { g: null, el: null },
  });
  const markers = useRef<(THREE.Group | null)[]>([]);
  const frame = useRef(0);
  const m4 = useMemo(() => new THREE.Matrix4(), []);
  const q = useMemo(() => new THREE.Quaternion(), []);

  useFrame(({ camera }) => {
    if (ready !== (on && skin.on)) setReady(on && skin.on);
    if (!on || !root.current) return;
    root.current.visible = skin.on;
    const st = useShip.getState();
    const s = shown();
    const info = st.info[st.scenario];
    if (!s || !info || !skin.on) return;
    const whole = !st.view.part;
    const cm = new THREE.Vector3(...toScene(s.position));
    const dist = camera.position.distanceTo(cm);
    const fov = (camera as THREE.PerspectiveCamera).fov;
    // Keep arrows a readable thickness and length on screen at any zoom.
    const unit = dist * Math.tan((fov * Math.PI) / 360);
    const w = unit * 0.0065;
    const forceScale = (unit * 0.42) / Math.max(s.weight, 1);
    const base = cm.clone().add(new THREE.Vector3(...toScene(rotate(s.quat, [0, 0, -info.ship.cm_height]))));

    const speed = Math.hypot(...s.velocity);
    setArrow(arrows.velocity, cm, toScene(s.velocity), Math.min(unit * 0.7, unit * 0.22 + speed * unit * 0.0009), w, whole);
    // The thrust arrow stays when the engines are open: it is their subject.
    setArrow(arrows.thrust, base, toScene(s.thrust_inertial), s.thrust_mag * forceScale, w * 1.3, whole || st.view.part === "engines");
    setArrow(arrows.aero, cm, toScene(s.aero_force), Math.hypot(...s.aero_force) * forceScale, w * 1.3, whole || st.view.part === "shield");
    setArrow(arrows.weight, cm, [0, -1, 0], s.weight * forceScale, w, whole || st.view.part === "tanks");

    // The body axis and the vertical through the centre of mass.
    const up = new THREE.Vector3(...toScene(rotate(s.quat, [0, 0, 1])));
    if (axis.current) {
      axis.current.position.copy(cm);
      axis.current.quaternion.setFromUnitVectors(Y, up);
      axis.current.scale.set(w * 0.35, info.ship.length * 1.5, w * 0.35);
    }
    if (plumb.current) {
      plumb.current.position.copy(cm);
      plumb.current.scale.set(w * 0.25, info.ship.length * 1.5, w * 0.25);
    }
    // The cone the thrust direction may move in (gimbal limit), at the base.
    if (cone.current) {
      const h = info.ship.length * 0.55;
      const r = h * Math.tan((info.tuning.gimbal_max_deg * Math.PI) / 180);
      cone.current.position.copy(base);
      cone.current.quaternion.setFromUnitVectors(Y, up.clone().negate());
      cone.current.scale.set(r, h, r);
      cone.current.visible = s.thrust_mag > 0 && (whole || st.view.part === "engines");
    }
    if (gate.current) {
      gate.current.visible = whole;
      gate.current.position.set(...toScene(info.gate));
      gate.current.scale.setScalar(Math.max(1, camera.position.distanceTo(gate.current.position) * 0.012));
    }
    if (ignition.current) {
      ignition.current.visible = whole && (s.phase === "belly-flop" || s.phase === "descent");
      ignition.current.position.set(cm.x, s.ignition_z, cm.z);
      ignition.current.scale.setScalar(unit * 0.35);
    }
    // The plan's nodes.
    const plan = shipLive.plan;
    if (nodes.current) {
      const n = plan && whole ? Math.min(plan.position.length, 32) : 0;
      for (let i = 0; i < n; i++) {
        const p = toScene(plan!.position[i]);
        m4.makeScale(w * 1.6, w * 1.6, w * 1.6).setPosition(p[0], p[1], p[2]);
        nodes.current.setMatrixAt(i, m4);
      }
      nodes.current.count = n;
      nodes.current.instanceMatrix.needsUpdate = true;
    }

    // Part markers ride on the vehicle.
    q.set(...quatToScene(s.quat));
    parts.forEach((p, i) => markers.current[i]?.position.set(p.at[0], p.at[1], p.at[2]).applyQuaternion(q).add(cm));

    // Labels: positions every frame, text a few times a second.
    const L = labels.current;
    const tip = (a: Arrow) => a.group.position.clone().add(new THREE.Vector3(0, a.head.position.y + 7 * w, 0).applyQuaternion(a.group.quaternion));
    for (const key of VECTORS) {
      const g = L[key].g;
      if (!g) continue;
      g.position.copy(tip(arrows[key]));
      if (L[key].el) L[key].el!.style.display = arrows[key].group.visible ? "" : "none";
    }
    L.tilt.g?.position.copy(cm).addScaledVector(up, info.ship.length * 0.8);
    L.gate.g?.position.set(...toScene(info.gate));
    if (L.tilt.el) L.tilt.el.style.display = whole ? "" : "none";
    if (L.gate.el) L.gate.el.style.display = whole ? "" : "none";
    frame.current += 1;
    if (frame.current % 6 === 0) {
      const set = (key: string, html: string) => {
        const el = L[key].el;
        if (el && el.innerHTML !== html) el.innerHTML = html;
      };
      set("velocity", `<b>v</b> ${fmt(speed)} m/s · M ${fmt(s.mach, 2)}`);
      set("thrust", `<b>T</b> ${fmt(s.thrust_mag / 1e6, 2)} MN · δ ${fmt(s.gimbal_deg, 1)}°`);
      set("aero", `<b>F<sub>aero</sub></b> ${fmt(Math.hypot(...s.aero_force) / 1e6, 2)} MN · q ${fmt(s.q_dyn / 1000, 1)} kPa`);
      set("weight", `<b>m·g</b> ${fmt(s.weight / 1e6, 2)} MN · m ${fmt(s.mass / 1000, 0)} t`);
      set("tilt", `<b>θ</b> ${fmt(s.tilt_deg, 1)}° · |ω| ${fmt((Math.hypot(...s.omega) * 180) / Math.PI, 1)}°/s`);
      set("gate", `<b>gate</b> ${fmt(Math.hypot(s.position[0] - info.gate[0], s.position[1] - info.gate[1], s.position[2] - info.gate[2]))} m`);
    }
  });

  if (!on) return null;
  const label = (key: string) => (
    <group ref={(g) => void (labels.current[key].g = g)}>
      {ready && (
        <Html center zIndexRange={[20, 0]} style={{ pointerEvents: "none" }}>
          <div className="an-label" data-kind={key} ref={(el) => void (labels.current[key].el = el)} />
        </Html>
      )}
    </group>
  );
  const line = { color: "#8fe9ff", transparent: true, opacity: 0.8, depthTest: false, fog: false } as const;
  return (
    <group ref={root} visible={false}>
      <primitive object={arrows.velocity.group} />
      <primitive object={arrows.thrust.group} />
      <primitive object={arrows.aero.group} />
      <primitive object={arrows.weight.group} />
      <mesh ref={axis} renderOrder={29} raycast={noRay}>
        <cylinderGeometry args={[1, 1, 1, 6]} />
        <meshBasicMaterial {...line} opacity={0.5} />
      </mesh>
      <mesh ref={plumb} renderOrder={29} raycast={noRay}>
        <cylinderGeometry args={[1, 1, 1, 6]} />
        <meshBasicMaterial {...line} opacity={0.25} />
      </mesh>
      <mesh ref={cone} renderOrder={28} raycast={noRay}>
        <coneGeometry args={[1, 1, 12, 1, true]} onUpdate={(g) => g.translate(0, -0.5, 0)} />
        <meshBasicMaterial color="#ffb35c" wireframe transparent opacity={0.3} depthTest={false} fog={false} toneMapped={false} />
      </mesh>
      <group ref={gate}>
        <mesh rotation-x={Math.PI / 2} renderOrder={28} raycast={noRay}>
          <torusGeometry args={[6, 0.25, 8, 48]} />
          <meshBasicMaterial {...line} />
        </mesh>
        <mesh rotation-x={Math.PI / 2} renderOrder={28} raycast={noRay}>
          <torusGeometry args={[1.2, 0.25, 8, 24]} />
          <meshBasicMaterial {...line} />
        </mesh>
      </group>
      <mesh ref={ignition} rotation-x={Math.PI / 2} renderOrder={28} raycast={noRay}>
        <ringGeometry args={[0.96, 1, 96]} />
        <meshBasicMaterial color="#ffb35c" transparent opacity={0.8} depthTest={false} side={THREE.DoubleSide} fog={false} />
      </mesh>
      <instancedMesh ref={nodes} args={[undefined, undefined, 32]} frustumCulled={false} renderOrder={29} raycast={noRay}>
        <sphereGeometry args={[1, 10, 8]} />
        <meshBasicMaterial color="#ffffff" depthTest={false} fog={false} />
      </instancedMesh>
      {label("velocity")}
      {label("thrust")}
      {label("aero")}
      {label("weight")}
      {label("tilt")}
      {label("gate")}
      {parts.map((p, i) => (
        <group key={p.id} ref={(g) => void (markers.current[i] = g)}>
          {ready && (
            <Html center zIndexRange={[19, 0]}>
              <button
                className="an-part"
                data-open={selected === p.id || undefined}
                data-dim={(selected !== null && selected !== p.id) || undefined}
                data-left={i % 2 === 1 || undefined}
                style={{ animationDelay: `${120 + i * 55}ms` }}
                onClick={() => setView({ part: selected === p.id ? null : p.id })}
                title={`Open ${p.name}`}
              >
                <i />
                <span>{p.short}</span>
              </button>
            </Html>
          )}
        </group>
      ))}
    </group>
  );
}
