use capability_common::serde_json::Value;
use get_employee_salary::get_salary;

fn main() {
    capability_common::run(|input: Value| {
        let employee_id = input
            .get("employee_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| capability_common::CapabilityError::new("Missing employee_id"))?;

        get_salary(employee_id)
    });
}
