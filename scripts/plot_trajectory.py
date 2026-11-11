"""Plot a Kestrel landing CSV.

Usage:
    python scripts/plot_trajectory.py target/landing.csv target/landing.png

Draws a 2x2 figure: path in the x-z plane with the glide-slope line, thrust
magnitude with its limits, tilt angle with its limit, and mass with the dry
mass. Limits default to the example vehicle; override with flags.
"""

import argparse
import csv
import math

import matplotlib

matplotlib.use("Agg")  # write files, no window
import matplotlib.pyplot as plt


def example_limits():
    t_max = 6 * 3100 * math.cos(math.radians(27))
    return {
        "min_thrust": 0.3 * t_max,
        "max_thrust": 0.8 * t_max,
        "max_tilt_deg": 45.0,
        "glide_slope_deg": 4.0,
        "dry_mass": 1505.0,
    }


def load(path):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f))
    cols = {key: [float(r[key]) if r[key] else None for r in rows] for key in rows[0]}
    return cols


def main():
    lim = example_limits()
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("csv")
    ap.add_argument("png")
    for key, value in lim.items():
        ap.add_argument("--" + key.replace("_", "-"), type=float, default=value)
    ap.add_argument("--title", default="Kestrel: fuel-optimal landing (3-DoF)")
    args = ap.parse_args()

    d = load(args.csv)
    t = d["t"]
    n_int = len(t) - 1  # thrust rows: one per interval
    tt = t[:n_int]
    thrust = d["thrust_mag"][:n_int]
    tilt = [
        math.degrees(math.acos(max(-1.0, min(1.0, tz / m)))) if m else 0.0
        for tz, m in zip(d["tz"][:n_int], thrust)
    ]

    fig, ax = plt.subplots(2, 2, figsize=(11, 8))
    fig.suptitle(args.title)

    a = ax[0][0]
    a.plot(d["x"], d["z"], "o-", ms=3, label="path")
    xmax = max(abs(x) for x in d["x"]) * 1.1
    gs = math.tan(math.radians(args.glide_slope_deg))
    a.plot([-xmax, 0, xmax], [gs * xmax, 0, gs * xmax], "k--", lw=1, label="glide slope")
    a.set_xlabel("x (m)")
    a.set_ylabel("altitude z (m)")
    a.set_title("Path")
    a.legend()

    a = ax[0][1]
    a.step(tt, thrust, where="post", label="‖T‖")
    a.axhline(args.min_thrust, color="k", ls="--", lw=1, label="ρ₁ (min)")
    a.axhline(args.max_thrust, color="k", ls=":", lw=1, label="ρ₂ (max)")
    a.set_xlabel("time (s)")
    a.set_ylabel("thrust (N)")
    a.set_title("Thrust magnitude")
    a.legend()

    a = ax[1][0]
    a.step(tt, tilt, where="post", label="tilt")
    a.axhline(args.max_tilt_deg, color="k", ls="--", lw=1, label="limit")
    a.set_xlabel("time (s)")
    a.set_ylabel("tilt from vertical (deg)")
    a.set_title("Thrust tilt")
    a.legend()

    a = ax[1][1]
    a.plot(t, d["mass"], label="mass")
    a.axhline(args.dry_mass, color="k", ls="--", lw=1, label="dry mass")
    a.set_xlabel("time (s)")
    a.set_ylabel("mass (kg)")
    a.set_title("Mass")
    a.legend()

    fig.tight_layout()
    fig.savefig(args.png, dpi=120)
    print(f"wrote {args.png}")


if __name__ == "__main__":
    main()
