//! Get Salary Details capability - returns salary and compensation data from the database.

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

        let salary = &employee.salary;
        Ok(json!({
            "employee_id": employee.employee_id,
            "base_salary": salary.base_salary,
            "currency": salary.currency,
            "pay_frequency": salary.pay_frequency,
            "bonus_target_percent": salary.bonus_target_percent,
            "last_bonus_amount": salary.last_bonus_amount,
            "stock_options": {
                "granted": salary.stock_options.granted,
                "vested": salary.stock_options.vested,
                "strike_price": salary.stock_options.strike_price
            },
            "deductions": {
                "health_insurance": salary.deductions.health_insurance,
                "dental": salary.deductions.dental,
                "vision": salary.deductions.vision,
                "retirement_contribution": salary.deductions.retirement_contribution
            },
            "last_raise_date": salary.last_raise_date,
            "last_raise_percent": salary.last_raise_percent
        }))
    });
}
