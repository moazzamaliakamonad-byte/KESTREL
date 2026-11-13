//! Print the bridge's response for the example request, as a test fixture
//! for the web app:
//!
//! cargo run -p kestrel-wasm --example fixture > web/src/lib/__fixtures__/example-response.json

fn main() {
    let response = kestrel_wasm::plan_landing_json(&kestrel_wasm::example_request_json());
    println!("{response}");
}
