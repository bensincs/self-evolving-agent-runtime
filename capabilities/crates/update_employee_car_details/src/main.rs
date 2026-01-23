//! Update Car Details capability - updates company car data in the database.

use capability_common::serde::{Deserialize, Serialize};
use capability_common::serde_json::json;
use capability_common::{CapabilityError, CompanyCar, EmployeeDatabase};

#[derive(Deserialize)]
struct UpdateCarInput {
    employee_id: String,
    make: Option<String>,
    model: Option<String>,
    year: Option<u16>,
    color: Option<String>,
    license_plate: Option<String>,
}

#[derive(Serialize)]
struct UpdateCarOutput {
    success: bool,
    message: String,
    updated_car: Option<CompanyCar>,
}

fn main() {
    capability_common::run(|input: UpdateCarInput| {
        let mut db = EmployeeDatabase::load();

        let employee = db.find_employee_mut(&input.employee_id).ok_or_else(|| {
            CapabilityError::new(format!("Employee not found: {}", input.employee_id))
        })?;

        // Check if employee is eligible for a company car
        if !employee.car.eligible {
            return Err(CapabilityError::new(format!(
                "Employee {} is not eligible for a company car",
                input.employee_id
            )));
        }

        // Get or create the company car
        let car = employee.car.company_car.get_or_insert_with(|| CompanyCar {
            make: String::new(),
            model: String::new(),
            year: 2024,
            color: String::new(),
            license_plate: String::new(),
            vin: String::new(),
            lease_start: String::new(),
            lease_end: String::new(),
            monthly_allowance: 0,
        });

        // Apply updates
        if let Some(make) = input.make {
            car.make = make;
        }
        if let Some(model) = input.model {
            car.model = model;
        }
        if let Some(year) = input.year {
            car.year = year;
        }
        if let Some(color) = input.color {
            car.color = color;
        }
        if let Some(license_plate) = input.license_plate {
            car.license_plate = license_plate;
        }

        // Clone the updated car for output before saving
        let updated_car = employee.car.company_car.clone();

        // Save the database
        db.save()
            .map_err(|e| CapabilityError::new(format!("Failed to save database: {}", e)))?;

        Ok(json!({
            "success": true,
            "message": format!("Updated car details for employee {}", input.employee_id),
            "updated_car": updated_car
        }))
    });
}
