use capability_common::serde_json::Value;
use cap_98446::greet;

fn main() {
    capability_common::run(|_input: Value| {
        Ok(greet())
    });
}
