# capability_common API Reference

This document describes the types and functions available in `capability_common` for writing capabilities.

## Core Concepts

**Every capability exports a `run` function** with this signature:

```rust
pub fn run(employee_id: &str, db: &EmployeeDatabase) -> Result<T, CapabilityError>
```

Where `T` is a **typed response struct** that implements `Serialize`.

## ⚠️ IMPORTANT: Use Typed Responses, NOT Raw JSON

**DO NOT** return raw `serde_json::Value`. Always define a typed response struct:

```rust
// ❌ BAD - avoid raw JSON
pub fn run(employee_id: &str, db: &EmployeeDatabase) -> Result<Value, CapabilityError> {
    Ok(json!({"name": "John"}))  // Type-unsafe!
}

// ✅ GOOD - use typed response
#[derive(Debug, Clone, Serialize)]
pub struct GetProfileResponse {
    pub employee_id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub department: String,
}

pub fn run(employee_id: &str, db: &EmployeeDatabase) -> Result<GetProfileResponse, CapabilityError> {
    let emp = db.find_employee(employee_id)
        .ok_or_else(|| CapabilityError::new(format!("Employee not found: {}", employee_id)))?;

    Ok(GetProfileResponse {
        employee_id: emp.employee_id.clone(),
        first_name: emp.profile.first_name.clone(),
        last_name: emp.profile.last_name.clone(),
        email: emp.profile.email.clone(),
        department: emp.profile.department.clone(),
    })
}
```

---

## Error Handling

```rust
use capability_common::CapabilityError;

// Create an error
let err = CapabilityError::new("Employee not found");

// Use with Result
fn run(id: &str, db: &EmployeeDatabase) -> Result<MyResponse, CapabilityError> {
    let emp = db.find_employee(id)
        .ok_or_else(|| CapabilityError::new(format!("Not found: {}", id)))?;
    // ...
}
```

---

## EmployeeDatabase

### Loading the Database

```rust
// In tests - use default_database() (works without host functions)
let db = EmployeeDatabase::default_database();

// In WASM runtime - use load() (reads from host filesystem)
let db = EmployeeDatabase::load();
```

### Finding Employees

```rust
// Returns Option<&Employee>
let employee = db.find_employee("EMP001");

// Available employee IDs in default database:
// - "EMP001" - John Smith (Engineering, Senior Software Engineer)
// - "EMP002" - Maria Garcia (Marketing, Marketing Manager)
// - "EMP003" - David Chen (Finance, Financial Analyst)
```

---

## Employee Type Structure

An `Employee` contains these fields:

```rust
pub struct Employee {
    pub employee_id: String,       // e.g., "EMP001"
    pub profile: EmployeeProfile,
    pub salary: SalaryDetails,
    pub hr_records: HrRecords,
    pub calendar: CalendarData,
    pub car: CarDetails,
    pub family: FamilyDetails,
    pub benefits: BenefitsInfo,
    pub leave: LeaveBalance,
    pub performance: PerformanceData,
    pub emergency_contacts: EmergencyContactData,
}
```

---

## EmployeeProfile

```rust
pub struct EmployeeProfile {
    pub first_name: String,      // e.g., "John"
    pub last_name: String,       // e.g., "Smith"
    pub email: String,           // e.g., "john.smith@company.com"
    pub phone: String,           // e.g., "+1-555-0123"
    pub department: String,      // e.g., "Engineering"
    pub job_title: String,       // e.g., "Senior Software Engineer"
    pub manager: String,         // e.g., "Jane Doe"
    pub location: String,        // e.g., "San Francisco, CA"
    pub start_date: String,      // e.g., "2020-03-15" (ISO date)
    pub status: String,          // e.g., "active"
}
```

---

## SalaryDetails

```rust
pub struct SalaryDetails {
    pub base_salary: u32,            // e.g., 145000
    pub currency: String,            // e.g., "USD"
    pub pay_frequency: String,       // e.g., "bi-weekly"
    pub bonus_target_percent: u8,    // e.g., 15
    pub last_bonus_amount: u32,      // e.g., 18000
    pub stock_options: StockOptions,
    pub deductions: Deductions,
    pub last_raise_date: String,     // e.g., "2025-01-01"
    pub last_raise_percent: f32,     // e.g., 8.5
}

pub struct StockOptions {
    pub granted: u32,        // e.g., 5000
    pub vested: u32,         // e.g., 2500
    pub strike_price: f32,   // e.g., 45.00
}

pub struct Deductions {
    pub health_insurance: u32,        // e.g., 450
    pub dental: u32,                  // e.g., 75
    pub vision: u32,                  // e.g., 25
    pub retirement_contribution: u32, // e.g., 1200
}
```

---

## CarDetails

```rust
pub struct CarDetails {
    pub eligible: bool,                      // true if employee qualifies
    pub company_car: Option<CompanyCar>,     // None if not assigned
    pub parking: Option<ParkingInfo>,
    pub mileage_log: Option<MileageLog>,
    pub fuel_card: Option<FuelCard>,
}

pub struct CompanyCar {
    pub make: String,           // e.g., "Tesla"
    pub model: String,          // e.g., "Model 3"
    pub year: u16,              // e.g., 2024
    pub color: String,          // e.g., "Midnight Silver"
    pub license_plate: String,  // e.g., "7ABC123"
    pub vin: String,            // e.g., "5YJ3E1EA1PF123456"
    pub lease_start: String,    // e.g., "2024-01-01"
    pub lease_end: String,      // e.g., "2027-01-01"
    pub monthly_allowance: u32, // e.g., 800
}

pub struct ParkingInfo {
    pub assigned_spot: String,  // e.g., "B-42"
    pub building: String,       // e.g., "HQ"
    pub level: String,          // e.g., "B1"
    pub ev_charging: bool,      // true if spot has EV charger
}

pub struct MileageLog {
    pub current_odometer: u32,     // e.g., 12500
    pub last_service_date: String, // e.g., "2025-10-15"
    pub next_service_due: String,  // e.g., "2026-04-15"
}

pub struct FuelCard {
    pub card_number: String,       // e.g., "****4567" (masked)
    pub monthly_limit: u32,        // e.g., 300
    pub current_month_spend: f32,  // e.g., 87.50
}
```

---

## FamilyDetails

```rust
pub struct FamilyDetails {
    pub marital_status: String,              // e.g., "married", "single"
    pub family_members: Vec<FamilyMember>,
    pub dependents_count: u8,                // e.g., 1
    pub benefits_tier: String,               // e.g., "family", "individual"
}

pub struct FamilyMember {
    pub relationship: String,       // e.g., "spouse", "child"
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: String,      // e.g., "1988-07-22"
    pub covered_by_benefits: bool,
    pub is_dependent: bool,
}
```

---

## LeaveBalance

```rust
pub struct LeaveBalance {
    pub pto: PtoBalance,
    pub sick_leave: LeaveType,
    pub personal_days: LeaveType,
    pub parental_leave: ParentalLeave,
    pub upcoming_time_off: Vec<TimeOffRequest>,
    pub holidays_remaining_this_year: u8,
    pub next_accrual_date: String,
    pub accrual_rate_per_month: f32,
}

pub struct PtoBalance {
    pub annual_allowance: u8,    // e.g., 25
    pub used: u8,                // e.g., 8
    pub remaining: u8,           // e.g., 17
    pub pending_approval: u8,    // e.g., 2
    pub carry_over_limit: u8,    // e.g., 5
}

pub struct LeaveType {
    pub annual_allowance: u8,
    pub used: u8,
    pub remaining: u8,
}

pub struct ParentalLeave {
    pub eligible: bool,
    pub weeks_available: u8,
    pub weeks_used: u8,
}

pub struct TimeOffRequest {
    pub start_date: String,
    pub end_date: String,
    pub leave_type: String,  // e.g., "PTO"
    pub status: String,      // e.g., "approved", "pending"
    pub days: u8,
}
```

---

## BenefitsInfo

```rust
pub struct BenefitsInfo {
    pub health_insurance: HealthInsurance,
    pub dental: DentalPlan,
    pub vision: VisionPlan,
    pub retirement: RetirementPlan,
    pub life_insurance: LifeInsurance,
    pub other_benefits: Vec<String>,
}

pub struct HealthInsurance {
    pub plan: String,             // e.g., "Premium PPO"
    pub provider: String,         // e.g., "Blue Cross Blue Shield"
    pub coverage_tier: String,    // e.g., "Family"
    pub monthly_premium: u32,     // e.g., 450
    pub deductible: u32,          // e.g., 1500
    pub out_of_pocket_max: u32,   // e.g., 6000
    pub policy_number: String,    // e.g., "BCBS-789456123"
}

pub struct DentalPlan {
    pub plan: String,
    pub provider: String,
    pub monthly_premium: u32,
    pub annual_max: u32,
}

pub struct VisionPlan {
    pub plan: String,
    pub provider: String,
    pub monthly_premium: u32,
    pub last_exam_date: String,
}

pub struct RetirementPlan {
    pub plan_type: String,           // e.g., "401k"
    pub contribution_percent: u8,    // e.g., 10
    pub employer_match_percent: u8,  // e.g., 6
    pub vested_percent: u8,          // e.g., 100
    pub current_balance: u32,        // e.g., 125000
}

pub struct LifeInsurance {
    pub coverage_amount: u32,     // e.g., 500000
    pub beneficiary: String,      // e.g., "Sarah Smith"
}
```

---

## PerformanceData

```rust
pub struct PerformanceData {
    pub reviews: Vec<PerformanceReview>,
    pub current_goals: Vec<Goal>,
    pub next_review_date: String,
}

pub struct PerformanceReview {
    pub review_period: String,          // e.g., "2025"
    pub overall_rating: f32,            // e.g., 4.5
    pub rating_scale: String,           // e.g., "1-5"
    pub performance_tier: String,       // e.g., "Exceeds Expectations"
    pub manager: String,
    pub strengths: Vec<String>,
    pub areas_for_improvement: Vec<String>,
    pub key_accomplishments: Vec<String>,
}

pub struct Goal {
    pub goal: String,
    pub target_date: String,
    pub progress_percent: u8,
}
```

---

## HrRecords

```rust
pub struct HrRecords {
    pub hire_date: String,
    pub employment_type: String,             // e.g., "full-time"
    pub contract_end_date: Option<String>,   // None for permanent
    pub promotions: Vec<Promotion>,
    pub certifications: Vec<Certification>,
    pub training_completed: Vec<String>,
    pub disciplinary_actions: Vec<String>,
    pub background_check_status: String,     // e.g., "cleared"
}

pub struct Promotion {
    pub date: String,
    pub from_title: String,
    pub to_title: String,
    pub salary_increase_percent: u8,
}

pub struct Certification {
    pub name: String,
    pub obtained_date: String,
    pub expiry_date: String,
}
```

---

## CalendarData

```rust
pub struct CalendarData {
    pub events: Vec<CalendarEvent>,
    pub out_of_office: Vec<OutOfOffice>,
    pub working_hours: WorkingHours,
}

pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start: String,       // ISO datetime e.g., "2026-01-23T09:00:00Z"
    pub end: String,
    pub location: String,
    pub recurring: bool,
    pub attendees: Vec<String>,
}

pub struct OutOfOffice {
    pub start: String,
    pub end: String,
    pub reason: String,
}

pub struct WorkingHours {
    pub start: String,    // e.g., "09:00"
    pub end: String,      // e.g., "17:00"
    pub timezone: String, // e.g., "America/Los_Angeles"
}
```

---

## EmergencyContactData

```rust
pub struct EmergencyContactData {
    pub contacts: Vec<EmergencyContact>,
    pub medical_info: MedicalInfo,
    pub last_updated: String,
}

pub struct EmergencyContact {
    pub priority: u8,                      // 1 = primary
    pub name: String,
    pub relationship: String,              // e.g., "Spouse"
    pub phone_primary: String,
    pub phone_secondary: Option<String>,
    pub email: String,
    pub address: String,
}

pub struct MedicalInfo {
    pub blood_type: String,          // e.g., "O+"
    pub allergies: Vec<String>,
    pub medications: Vec<String>,
    pub medical_conditions: Vec<String>,
    pub physician_name: String,
    pub physician_phone: String,
}
```

---

## Example Capability: get_car_details

```rust
// src/lib.rs
use capability_common::{EmployeeDatabase, CapabilityError};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CarDetailsResponse {
    pub employee_id: String,
    pub eligible: bool,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<u16>,
    pub license_plate: Option<String>,
    pub parking_spot: Option<String>,
}

pub fn run(employee_id: &str, db: &EmployeeDatabase) -> Result<CarDetailsResponse, CapabilityError> {
    let emp = db.find_employee(employee_id)
        .ok_or_else(|| CapabilityError::new(format!("Employee not found: {}", employee_id)))?;

    let car = &emp.car;

    Ok(CarDetailsResponse {
        employee_id: emp.employee_id.clone(),
        eligible: car.eligible,
        make: car.company_car.as_ref().map(|c| c.make.clone()),
        model: car.company_car.as_ref().map(|c| c.model.clone()),
        year: car.company_car.as_ref().map(|c| c.year),
        license_plate: car.company_car.as_ref().map(|c| c.license_plate.clone()),
        parking_spot: car.parking.as_ref().map(|p| p.assigned_spot.clone()),
    })
}
```

```rust
// src/main.rs
use capability_common::serde_json::Value;
use capability_common::EmployeeDatabase;
use get_car_details::run;

fn main() {
    capability_common::run(|input: Value| {
        let employee_id = input.get("employee_id").and_then(|v| v.as_str()).unwrap_or("EMP001");
        let db = EmployeeDatabase::load();
        run(employee_id, &db)
    });
}
```

```rust
// tests/integration.rs
use capability_common::EmployeeDatabase;
use get_car_details::{run, CarDetailsResponse};

fn test_db() -> EmployeeDatabase {
    EmployeeDatabase::default_database()  // Use default in tests!
}

#[test]
fn test_car_details_for_eligible_employee() {
    let db = test_db();
    let result: CarDetailsResponse = run("EMP001", &db).expect("should succeed");

    assert_eq!(result.employee_id, "EMP001");
    assert!(result.eligible);
    assert_eq!(result.make, Some("Tesla".to_string()));
    assert_eq!(result.model, Some("Model 3".to_string()));
    assert_eq!(result.year, Some(2024));
}

#[test]
fn test_car_details_for_ineligible_employee() {
    let db = test_db();
    let result = run("EMP002", &db).expect("should succeed");

    assert!(!result.eligible);
    assert!(result.make.is_none());
}

#[test]
fn test_unknown_employee_returns_error() {
    let db = test_db();
    let result = run("UNKNOWN", &db);

    assert!(result.is_err());
}
```

---

## Re-exports

The following are re-exported for convenience:

```rust
pub use serde;        // For derive macros
pub use serde_json;   // For JSON operations if needed
```

To use Serialize derive macro, just import directly since serde is in your Cargo.toml:
```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct MyResponse {
    pub field: String,
}
```
