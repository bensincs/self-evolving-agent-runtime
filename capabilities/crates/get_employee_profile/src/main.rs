use capability_common::serde_json::Value;
use get_employee_profile::get_profile;

fn main() {
    capability_common::run(|input: Value| {
        let employee_id = input
            .get("employee_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| capability_common::CapabilityError::new("Missing employee_id"))?;

        get_profile(employee_id)
    });
}
