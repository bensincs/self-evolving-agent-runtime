//! Get Car Details capability - returns company car data from the database.

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

        let car = &employee.car;
        Ok(json!({
            "employee_id": employee.employee_id,
            "eligible": car.eligible,
            "company_car": car.company_car.as_ref().map(|c| json!({
                "make": c.make,
                "model": c.model,
                "year": c.year,
                "color": c.color,
                "license_plate": c.license_plate,
                "vin": c.vin,
                "lease_start": c.lease_start,
                "lease_end": c.lease_end,
                "monthly_allowance": c.monthly_allowance
            })),
            "parking": car.parking.as_ref().map(|p| json!({
                "assigned_spot": p.assigned_spot,
                "building": p.building,
                "level": p.level,
                "ev_charging": p.ev_charging
            })),
            "mileage_log": car.mileage_log.as_ref().map(|m| json!({
                "current_odometer": m.current_odometer,
                "last_service_date": m.last_service_date,
                "next_service_due": m.next_service_due
            })),
            "fuel_card": car.fuel_card.as_ref().map(|f| json!({
                "card_number": f.card_number,
                "monthly_limit": f.monthly_limit,
                "current_month_spend": f.current_month_spend
            }))
        }))
    });
}
