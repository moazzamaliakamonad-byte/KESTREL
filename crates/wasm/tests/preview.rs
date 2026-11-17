//! The page's prediction: a whole flight flown ahead of time, noise off,
//! every plan arriving at once (`preview_ship`). A plan built from a state
//! one control step old (stamped "now") once made the booster's preview
//! fail 275 of 303 plans and the ship's hit the tower; this guards it.
//! Simulation only.

use kestrel_wasm::ship_js::preview_ship;
use serde_json::Value;

fn preview(config: &str) -> Value {
    serde_json::from_str(&preview_ship(config)).expect("json")
}

#[test]
fn booster_preview_is_caught() {
    let r = preview(
        r#"{"scenario":"booster","position":[900,0,14000],"velocity":[-30,0,-420],"lag":0.3}"#,
    );
    assert_eq!(r["outcome"], "caught", "{r:?}");
}

#[test]
fn ship_preview_is_caught() {
    let r =
        preview(r#"{"scenario":"earth","position":[700,0,5000],"velocity":[-25,0,-75],"lag":0.3}"#);
    assert_eq!(r["outcome"], "caught", "{r:?}");
}
