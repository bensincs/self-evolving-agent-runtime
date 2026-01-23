//! Get Emergency Contacts capability - returns emergency contact data from the database.

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

        let ec = &employee.emergency_contacts;
        Ok(json!({
            "employee_id": employee.employee_id,
            "emergency_contacts": ec.contacts.iter().map(|c| json!({
                "priority": c.priority,
                "name": c.name,
                "relationship": c.relationship,
                "phone_primary": c.phone_primary,
                "phone_secondary": c.phone_secondary,
                "email": c.email,
                "address": c.address
            })).collect::<Vec<_>>(),
            "medical_info": {
                "blood_type": ec.medical_info.blood_type,
                "allergies": ec.medical_info.allergies,
                "medications": ec.medical_info.medications,
                "medical_conditions": ec.medical_info.medical_conditions,
                "physician_name": ec.medical_info.physician_name,
                "physician_phone": ec.medical_info.physician_phone
            },
            "last_updated": ec.last_updated
        }))
    });
}
