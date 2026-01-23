//! Get Family Details capability - returns family member data from the database.

use capability_common::{EmployeeDatabase, CapabilityError};
use capability_common::serde_json::{json, Value};

fn main() {
    capability_common::run(|input: Value| {
        let employee_id = input.get("employee_id")
            .and_then(|v| v.as_str())
            .unwrap_or("EMP001");

        let db = EmployeeDatabase::load();
        let employee = db.find_employee(employee_id)
            .ok_or_else(|| CapabilityError::new(format!("Employee not found: {}", employee_id)))?;

        let family = &employee.family;
        Ok(json!({
            "employee_id": employee.employee_id,
            "marital_status": family.marital_status,
            "family_members": family.family_members.iter().map(|m| json!({
                "relationship": m.relationship,
                "first_name": m.first_name,
                "last_name": m.last_name,
                "date_of_birth": m.date_of_birth,
                "covered_by_benefits": m.covered_by_benefits,
                "is_dependent": m.is_dependent
            })).collect::<Vec<_>>(),
            "dependents_count": family.dependents_count,
            "benefits_tier": family.benefits_tier
        }))
    });
}
