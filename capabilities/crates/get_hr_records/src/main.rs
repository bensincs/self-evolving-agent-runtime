//! Get HR Records capability - returns HR records data from the database.

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

        let hr = &employee.hr_records;
        Ok(json!({
            "employee_id": employee.employee_id,
            "hire_date": hr.hire_date,
            "employment_type": hr.employment_type,
            "contract_end_date": hr.contract_end_date,
            "promotions": hr.promotions.iter().map(|p| json!({
                "date": p.date,
                "from_title": p.from_title,
                "to_title": p.to_title,
                "salary_increase_percent": p.salary_increase_percent
            })).collect::<Vec<_>>(),
            "certifications": hr.certifications.iter().map(|c| json!({
                "name": c.name,
                "obtained_date": c.obtained_date,
                "expiry_date": c.expiry_date
            })).collect::<Vec<_>>(),
            "training_completed": hr.training_completed,
            "disciplinary_actions": hr.disciplinary_actions,
            "background_check_status": hr.background_check_status
        }))
    });
}
