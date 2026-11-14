//! JSON shapes for the bridge. Kept separate so `guidance` stays free of
//! serialization concerns.

use guidance::{Trajectory, Vec3, Vehicle};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleDto {
    pub wet_mass: f64,
    pub dry_mass: f64,
    pub isp: f64,
    pub min_thrust: f64,
    pub max_thrust: f64,
    pub gravity: f64,
    pub glide_slope: f64,
    pub max_tilt: f64,
}

impl From<&Vehicle> for VehicleDto {
    fn from(v: &Vehicle) -> Self {
        VehicleDto {
            wet_mass: v.wet_mass,
            dry_mass: v.dry_mass,
            isp: v.isp,
            min_thrust: v.min_thrust,
            max_thrust: v.max_thrust,
            gravity: v.gravity,
            glide_slope: v.glide_slope,
            max_tilt: v.max_tilt,
        }
    }
}

impl From<VehicleDto> for Vehicle {
    fn from(d: VehicleDto) -> Self {
        Vehicle {
            wet_mass: d.wet_mass,
            dry_mass: d.dry_mass,
            isp: d.isp,
            min_thrust: d.min_thrust,
            max_thrust: d.max_thrust,
            gravity: d.gravity,
            glide_slope: d.glide_slope,
            max_tilt: d.max_tilt,
        }
    }
}

/// What the page sends. `steps` and `vehicle` are optional.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestDto {
    pub position: Vec3,
    pub velocity: Vec3,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<VehicleDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryDto {
    pub t: Vec<f64>,
    pub position: Vec<Vec3>,
    pub velocity: Vec<Vec3>,
    pub mass: Vec<f64>,
    pub thrust: Vec<Vec3>,
    pub final_time: f64,
    pub fuel_used: f64,
}

impl From<Trajectory> for TrajectoryDto {
    fn from(t: Trajectory) -> Self {
        let fuel_used = t.fuel_used();
        TrajectoryDto {
            t: t.t,
            position: t.position,
            velocity: t.velocity,
            mass: t.mass,
            thrust: t.thrust,
            final_time: t.final_time,
            fuel_used,
        }
    }
}

impl From<TrajectoryDto> for Trajectory {
    fn from(d: TrajectoryDto) -> Self {
        Trajectory {
            t: d.t,
            position: d.position,
            velocity: d.velocity,
            mass: d.mass,
            thrust: d.thrust,
            final_time: d.final_time,
        }
    }
}
