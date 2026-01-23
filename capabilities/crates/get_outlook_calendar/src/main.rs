//! Get Outlook Calendar capability - returns calendar events from the database.

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

        let calendar = &employee.calendar;
        Ok(json!({
            "employee_id": employee.employee_id,
            "calendar_events": calendar.events.iter().map(|e| json!({
                "id": e.id,
                "title": e.title,
                "start": e.start,
                "end": e.end,
                "location": e.location,
                "recurring": e.recurring,
                "attendees": e.attendees
            })).collect::<Vec<_>>(),
            "out_of_office": calendar.out_of_office.iter().map(|o| json!({
                "start": o.start,
                "end": o.end,
                "reason": o.reason
            })).collect::<Vec<_>>(),
            "working_hours": {
                "start": calendar.working_hours.start,
                "end": calendar.working_hours.end,
                "timezone": calendar.working_hours.timezone
            }
        }))
    });
}
