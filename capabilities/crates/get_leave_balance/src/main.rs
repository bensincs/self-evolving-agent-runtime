//! Get Leave Balance capability - returns PTO and leave data from the database.

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

        let leave = &employee.leave;
        Ok(json!({
            "employee_id": employee.employee_id,
            "pto": {
                "annual_allowance": leave.pto.annual_allowance,
                "used": leave.pto.used,
                "remaining": leave.pto.remaining,
                "pending_approval": leave.pto.pending_approval,
                "carry_over_limit": leave.pto.carry_over_limit
            },
            "sick_leave": {
                "annual_allowance": leave.sick_leave.annual_allowance,
                "used": leave.sick_leave.used,
                "remaining": leave.sick_leave.remaining
            },
            "personal_days": {
                "annual_allowance": leave.personal_days.annual_allowance,
                "used": leave.personal_days.used,
                "remaining": leave.personal_days.remaining
            },
            "parental_leave": {
                "eligible": leave.parental_leave.eligible,
                "weeks_available": leave.parental_leave.weeks_available,
                "weeks_used": leave.parental_leave.weeks_used
            },
            "upcoming_time_off": leave.upcoming_time_off.iter().map(|t| json!({
                "start_date": t.start_date,
                "end_date": t.end_date,
                "type": t.leave_type,
                "status": t.status,
                "days": t.days
            })).collect::<Vec<_>>(),
            "holidays_remaining_this_year": leave.holidays_remaining_this_year,
            "next_accrual_date": leave.next_accrual_date,
            "accrual_rate_per_month": leave.accrual_rate_per_month
        }))
    });
}
