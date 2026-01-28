use capability_common::serde_json::Value;
use cap_98549::{update_salary, UpdateSalaryInput};

fn main() {
    capability_common::run(|input: Value| {
        let parsed: UpdateSalaryInput = serde_json::from_value(input)
            .map_err(|e| capability_common::CapabilityError::new(e.to_string()))?;
        update_salary(parsed)
    });
}
