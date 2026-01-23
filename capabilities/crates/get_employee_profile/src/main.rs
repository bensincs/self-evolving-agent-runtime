//! Get Employee Profile capability - returns employee profile data from the database.

use capability_common::serde_json::{json, Value};
use capability_common::{CapabilityError, EmployeeDatabase};

fn main() {
    capability_common::run(|input: Value| {
        let employee_id = input
            .get("employee_id")
            .and_then(|v| v.as_str())
            .unwrap_or("EMP001");

        let db = EmployeeDatabase::load();
        let employee = db
            .find_employee(employee_id)
            .ok_or_else(|| CapabilityError::new(format!("Employee not found: {}", employee_id)))?;

        let profile = &employee.profile;
        Ok(json!({
            "employee_id": employee.employee_id,
            "first_name": profile.first_name,
            "last_name": profile.last_name,
            "email": profile.email,
            "phone": profile.phone,
            "department": profile.department,
            "job_title": profile.job_title,
            "manager": profile.manager,
            "location": profile.location,
            "start_date": profile.start_date,
            "status": profile.status
        }))
    });
}
