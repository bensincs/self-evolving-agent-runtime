use capability_common::serde_json::json;

fn main() {
    capability_common::run(|_input: capability_common::serde_json::Value| {
        let iso_time = capability_common::utc_now_iso8601();
        let output = json!({"time": iso_time});
        Ok(output)
    });
}