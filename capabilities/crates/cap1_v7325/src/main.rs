use capability_common::serde_json::Value;
use cap1_v7325::run;

fn main() {
    capability_common::run(|input: Value| {
        let a = input.get("a").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let b = input.get("b").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        Ok(run(a, b))
    });
}
