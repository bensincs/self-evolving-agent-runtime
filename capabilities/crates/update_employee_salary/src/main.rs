//! Update Employee Salary capability - updates an employee's base salary.

use capability_common::serde_json::{json, Value};
use capability_common::{CapabilityError, EmployeeDatabase};

fn main() {
    capability_common::run(|input: Value| {
        let employee_id = input
            .get("employee_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CapabilityError::new("Missing employee_id"))?;

        let new_salary_usd = input
            .get("new_salary_usd")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| CapabilityError::new("Missing new_salary_usd"))?;

        let mut db = EmployeeDatabase::load();
        {
            let employee = db
                .find_employee_mut(employee_id)
                .ok_or_else(|| CapabilityError::new(format!("Employee not found: {}", employee_id)))?;

            // Update the employee's salary
            employee.salary.base_salary = new_salary_usd as u32;
        } // The mutable borrow ends here

        // Save the updated employee database
        db.save()?;

        Ok(json!({
            "employee_id": employee_id,
            "updated_salary_usd": new_salary_usd
        }))
    });
}