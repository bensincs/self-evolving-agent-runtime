//! Common utilities for self-evolving agent WASM capabilities.
//!
//! This crate provides helpers for:
//! - Reading JSON input from stdin
//! - Writing JSON output to stdout
//! - Making HTTP requests (via host functions)
//! - Getting current time (via host functions)
//! - Error handling patterns
//! - Mock employee database for testing

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::Read;

// ============ Host Function Imports ============
// These are implemented by the runtime host (CapabilityRunner)
// Only available when compiling for WASM target

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "host")]
extern "C" {
    /// Make an HTTP GET request.
    /// url_ptr: pointer to URL string
    /// url_len: length of URL string
    /// result_ptr: pointer to buffer for response body
    /// Returns: length of response written, or negative error code
    fn http_get(url_ptr: *const u8, url_len: i32, result_ptr: *mut u8) -> i32;

    /// Get current time in milliseconds since Unix epoch.
    fn current_time_millis() -> i64;

    /// Get current time in seconds since Unix epoch.
    fn current_time_secs() -> i64;

    /// Read a file from the host filesystem.
    /// path_ptr: pointer to file path string
    /// path_len: length of file path string
    /// result_ptr: pointer to buffer for file contents
    /// Returns: length of content written, or negative error code
    fn file_read(path_ptr: *const u8, path_len: i32, result_ptr: *mut u8) -> i32;

    /// Write content to a file on the host filesystem.
    /// path_ptr: pointer to file path string
    /// path_len: length of file path string
    /// content_ptr: pointer to content to write
    /// content_len: length of content to write
    /// Returns: 0 on success, or negative error code
    fn file_write(
        path_ptr: *const u8,
        path_len: i32,
        content_ptr: *const u8,
        content_len: i32,
    ) -> i32;
}

// ============ Native Stubs (for testing) ============
// These panic at runtime but allow tests to compile

#[cfg(not(target_arch = "wasm32"))]
unsafe fn http_get(_url_ptr: *const u8, _url_len: i32, _result_ptr: *mut u8) -> i32 {
    panic!("http_get is only available in WASM runtime")
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn file_read(_path_ptr: *const u8, _path_len: i32, _result_ptr: *mut u8) -> i32 {
    panic!("file_read is only available in WASM runtime")
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn file_write(
    _path_ptr: *const u8,
    _path_len: i32,
    _content_ptr: *const u8,
    _content_len: i32,
) -> i32 {
    panic!("file_write is only available in WASM runtime")
}

// ============ Error Type ============

/// Error type for capability operations.
#[derive(Debug, Serialize)]
pub struct CapabilityError {
    pub error: String,
}

impl CapabilityError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { error: msg.into() }
    }
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl std::error::Error for CapabilityError {}

// ============ Input/Output Helpers ============

/// Read and parse JSON input from stdin.
pub fn read_input<T: DeserializeOwned>() -> Result<T, CapabilityError> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| CapabilityError::new(format!("Failed to read stdin: {}", e)))?;

    serde_json::from_str(&input)
        .map_err(|e| CapabilityError::new(format!("Invalid JSON input: {}", e)))
}

/// Read raw JSON value from stdin.
pub fn read_input_value() -> Result<serde_json::Value, CapabilityError> {
    read_input()
}

/// Write a successful JSON response to stdout.
pub fn write_output<T: Serialize>(output: &T) {
    match serde_json::to_string(output) {
        Ok(json) => println!("{}", json),
        Err(e) => write_error(&format!("Failed to serialize output: {}", e)),
    }
}

/// Write an error response to stdout as JSON.
pub fn write_error(msg: &str) {
    let err = CapabilityError::new(msg);
    println!("{}", serde_json::to_string(&err).unwrap());
}

/// Run a capability with automatic error handling.
pub fn run<I, O, F>(handler: F)
where
    I: DeserializeOwned,
    O: Serialize,
    F: FnOnce(I) -> Result<O, CapabilityError>,
{
    match read_input::<I>() {
        Ok(input) => match handler(input) {
            Ok(output) => write_output(&output),
            Err(e) => write_error(&e.error),
        },
        Err(e) => write_error(&e.error),
    }
}

// ============ HTTP Functions (via host) ============

// Buffer size for HTTP responses (1MB)
const HTTP_BUFFER_SIZE: usize = 1024 * 1024;

/// Make an HTTP GET request and return the response body as a string.
///
/// # Example
/// ```ignore
/// let body = capability_common::http_get_string("https://api.example.com/data")?;
/// ```
pub fn http_get_string(url: &str) -> Result<String, CapabilityError> {
    let url_bytes = url.as_bytes();
    let mut buffer = vec![0u8; HTTP_BUFFER_SIZE];

    let result = unsafe {
        http_get(
            url_bytes.as_ptr(),
            url_bytes.len() as i32,
            buffer.as_mut_ptr(),
        )
    };

    if result < 0 {
        let error_msg = match result {
            -1 => "Memory export not found",
            -2 => "URL pointer out of bounds",
            -3 => "Invalid URL encoding",
            -4 => "HTTP request failed",
            -5 => "Failed to read response body",
            -6 => "Response buffer too small",
            _ => "Unknown error",
        };
        return Err(CapabilityError::new(format!(
            "HTTP GET failed: {}",
            error_msg
        )));
    }

    let len = result as usize;
    buffer.truncate(len);

    String::from_utf8(buffer)
        .map_err(|e| CapabilityError::new(format!("Response not valid UTF-8: {}", e)))
}

/// Make an HTTP GET request and parse the response as JSON.
///
/// # Example
/// ```ignore
/// #[derive(Deserialize)]
/// struct Weather { temp: f64 }
///
/// let weather: Weather = capability_common::http_get_json("https://wttr.in/London?format=j1")?;
/// ```
pub fn http_get_json<T: DeserializeOwned>(url: &str) -> Result<T, CapabilityError> {
    let body = http_get_string(url)?;
    serde_json::from_str(&body)
        .map_err(|e| CapabilityError::new(format!("Failed to parse JSON: {}", e)))
}

// ============ Time Functions (via host) ============

/// Get the current UTC time as Unix timestamp in milliseconds.
pub fn utc_now_timestamp_millis() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { current_time_millis() }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        current_time_millis()
    }
}

/// Get the current UTC time as Unix timestamp in seconds.
pub fn utc_now_timestamp() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { current_time_secs() }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        current_time_secs()
    }
}

/// Get the current UTC time as an ISO 8601 formatted string.
///
/// Returns format: "2024-01-15T10:30:00Z"
///
/// # Example
/// ```ignore
/// let iso_time = capability_common::utc_now_iso8601();
/// // Returns something like "2024-01-15T10:30:00Z"
/// ```
pub fn utc_now_iso8601() -> String {
    let secs = utc_now_timestamp();
    timestamp_to_iso8601(secs)
}

/// Convert a Unix timestamp (seconds) to an ISO 8601 formatted string.
///
/// Returns format: "2024-01-15T10:30:00Z"
///
/// # Example
/// ```ignore
/// let iso_time = capability_common::timestamp_to_iso8601(1705315800);
/// // Returns "2024-01-15T10:30:00Z"
/// ```
pub fn timestamp_to_iso8601(timestamp_secs: i64) -> String {
    // Calculate date/time components from Unix timestamp
    // Days since epoch
    let days = timestamp_secs / 86400;
    let remaining_secs = timestamp_secs % 86400;

    let hours = remaining_secs / 3600;
    let minutes = (remaining_secs % 3600) / 60;
    let seconds = remaining_secs % 60;

    // Calculate year, month, day from days since 1970-01-01
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    // Algorithm based on Howard Hinnant's date algorithms
    // http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    (y as i32, m, d)
}

// Re-export commonly used items
pub use serde;
pub use serde_json;

// ============ File I/O Functions (via host) ============

// Buffer size for file reads (4MB)
const FILE_BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Default path for the employee database file.
pub const EMPLOYEE_DB_PATH: &str = "employee_database.json";

/// Read a file from the host filesystem.
///
/// # Example
/// ```ignore
/// let contents = capability_common::read_file_string("config.json")?;
/// ```
pub fn read_file_string(path: &str) -> Result<String, CapabilityError> {
    let path_bytes = path.as_bytes();
    let mut buffer = vec![0u8; FILE_BUFFER_SIZE];

    let result = unsafe {
        file_read(
            path_bytes.as_ptr(),
            path_bytes.len() as i32,
            buffer.as_mut_ptr(),
        )
    };

    if result < 0 {
        let error_msg = match result {
            -1 => "Memory export not found",
            -2 => "Path pointer out of bounds",
            -3 => "Invalid path encoding",
            -4 => "File not found",
            -5 => "Permission denied",
            -6 => "Failed to read file",
            -7 => "File too large for buffer",
            _ => "Unknown error",
        };
        return Err(CapabilityError::new(format!(
            "File read failed: {}",
            error_msg
        )));
    }

    let len = result as usize;
    buffer.truncate(len);

    String::from_utf8(buffer)
        .map_err(|e| CapabilityError::new(format!("File not valid UTF-8: {}", e)))
}

/// Read and parse a JSON file.
pub fn read_file_json<T: DeserializeOwned>(path: &str) -> Result<T, CapabilityError> {
    let contents = read_file_string(path)?;
    serde_json::from_str(&contents)
        .map_err(|e| CapabilityError::new(format!("Failed to parse JSON file: {}", e)))
}

/// Write a string to a file on the host filesystem.
///
/// # Example
/// ```ignore
/// capability_common::write_file_string("output.txt", "Hello, world!")?;
/// ```
pub fn write_file_string(path: &str, content: &str) -> Result<(), CapabilityError> {
    let path_bytes = path.as_bytes();
    let content_bytes = content.as_bytes();

    let result = unsafe {
        file_write(
            path_bytes.as_ptr(),
            path_bytes.len() as i32,
            content_bytes.as_ptr(),
            content_bytes.len() as i32,
        )
    };

    if result < 0 {
        let error_msg = match result {
            -1 => "Memory export not found",
            -2 => "Path pointer out of bounds",
            -3 => "Invalid path encoding",
            -4 => "Content pointer out of bounds",
            -5 => "Permission denied",
            -6 => "Failed to write file",
            _ => "Unknown error",
        };
        return Err(CapabilityError::new(format!(
            "File write failed: {}",
            error_msg
        )));
    }

    Ok(())
}

/// Write a value as JSON to a file.
pub fn write_file_json<T: Serialize>(path: &str, value: &T) -> Result<(), CapabilityError> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| CapabilityError::new(format!("Failed to serialize to JSON: {}", e)))?;
    write_file_string(path, &json)
}

// ============ Employee Database ============

/// Mock employee database with 3 employees for testing capabilities.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeDatabase {
    pub employees: Vec<Employee>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Employee {
    pub employee_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeProfile {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub department: String,
    pub job_title: String,
    pub manager: String,
    pub location: String,
    pub start_date: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalaryDetails {
    pub base_salary: u32,
    pub currency: String,
    pub pay_frequency: String,
    pub bonus_target_percent: u8,
    pub last_bonus_amount: u32,
    pub stock_options: StockOptions,
    pub deductions: Deductions,
    pub last_raise_date: String,
    pub last_raise_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockOptions {
    pub granted: u32,
    pub vested: u32,
    pub strike_price: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deductions {
    pub health_insurance: u32,
    pub dental: u32,
    pub vision: u32,
    pub retirement_contribution: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrRecords {
    pub hire_date: String,
    pub employment_type: String,
    pub contract_end_date: Option<String>,
    pub promotions: Vec<Promotion>,
    pub certifications: Vec<Certification>,
    pub training_completed: Vec<String>,
    pub disciplinary_actions: Vec<String>,
    pub background_check_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Promotion {
    pub date: String,
    pub from_title: String,
    pub to_title: String,
    pub salary_increase_percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certification {
    pub name: String,
    pub obtained_date: String,
    pub expiry_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarData {
    pub events: Vec<CalendarEvent>,
    pub out_of_office: Vec<OutOfOffice>,
    pub working_hours: WorkingHours,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start: String,
    pub end: String,
    pub location: String,
    pub recurring: bool,
    pub attendees: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutOfOffice {
    pub start: String,
    pub end: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingHours {
    pub start: String,
    pub end: String,
    pub timezone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarDetails {
    pub eligible: bool,
    pub company_car: Option<CompanyCar>,
    pub parking: Option<ParkingInfo>,
    pub mileage_log: Option<MileageLog>,
    pub fuel_card: Option<FuelCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyCar {
    pub make: String,
    pub model: String,
    pub year: u16,
    pub color: String,
    pub license_plate: String,
    pub vin: String,
    pub lease_start: String,
    pub lease_end: String,
    pub monthly_allowance: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParkingInfo {
    pub assigned_spot: String,
    pub building: String,
    pub level: String,
    pub ev_charging: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MileageLog {
    pub current_odometer: u32,
    pub last_service_date: String,
    pub next_service_due: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelCard {
    pub card_number: String,
    pub monthly_limit: u32,
    pub current_month_spend: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyDetails {
    pub marital_status: String,
    pub family_members: Vec<FamilyMember>,
    pub dependents_count: u8,
    pub benefits_tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyMember {
    pub relationship: String,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: String,
    pub covered_by_benefits: bool,
    pub is_dependent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenefitsInfo {
    pub health_insurance: HealthInsurance,
    pub dental: DentalPlan,
    pub vision: VisionPlan,
    pub retirement: RetirementPlan,
    pub life_insurance: LifeInsurance,
    pub other_benefits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInsurance {
    pub plan: String,
    pub provider: String,
    pub coverage_tier: String,
    pub monthly_premium: u32,
    pub deductible: u32,
    pub out_of_pocket_max: u32,
    pub policy_number: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DentalPlan {
    pub plan: String,
    pub provider: String,
    pub monthly_premium: u32,
    pub annual_max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionPlan {
    pub plan: String,
    pub provider: String,
    pub monthly_premium: u32,
    pub last_exam_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetirementPlan {
    pub plan_type: String,
    pub contribution_percent: u8,
    pub employer_match_percent: u8,
    pub vested_percent: u8,
    pub current_balance: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeInsurance {
    pub coverage_amount: u32,
    pub beneficiary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtoBalance {
    pub annual_allowance: u8,
    pub used: u8,
    pub remaining: u8,
    pub pending_approval: u8,
    pub carry_over_limit: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveType {
    pub annual_allowance: u8,
    pub used: u8,
    pub remaining: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentalLeave {
    pub eligible: bool,
    pub weeks_available: u8,
    pub weeks_used: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeOffRequest {
    pub start_date: String,
    pub end_date: String,
    pub leave_type: String,
    pub status: String,
    pub days: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceData {
    pub reviews: Vec<PerformanceReview>,
    pub current_goals: Vec<Goal>,
    pub next_review_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReview {
    pub review_period: String,
    pub overall_rating: f32,
    pub rating_scale: String,
    pub performance_tier: String,
    pub manager: String,
    pub strengths: Vec<String>,
    pub areas_for_improvement: Vec<String>,
    pub key_accomplishments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub goal: String,
    pub target_date: String,
    pub progress_percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyContactData {
    pub contacts: Vec<EmergencyContact>,
    pub medical_info: MedicalInfo,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyContact {
    pub priority: u8,
    pub name: String,
    pub relationship: String,
    pub phone_primary: String,
    pub phone_secondary: Option<String>,
    pub email: String,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicalInfo {
    pub blood_type: String,
    pub allergies: Vec<String>,
    pub medications: Vec<String>,
    pub medical_conditions: Vec<String>,
    pub physician_name: String,
    pub physician_phone: String,
}

impl EmployeeDatabase {
    /// Load the employee database from the JSON file.
    /// Falls back to default data if file doesn't exist or can't be read.
    ///
    /// Behavior can be overridden for tests by setting `EMPLOYEE_DB_PATH` env var.
    pub fn load() -> Self {
        if let Ok(path) = std::env::var("EMPLOYEE_DB_PATH") {
            return Self::load_from_file(&path).unwrap_or_else(|_| Self::default_database());
        }
        Self::load_from_file(EMPLOYEE_DB_PATH).unwrap_or_else(|_| Self::default_database())
    }

    /// Load the employee database from a specific file path.
    pub fn load_from_file(path: &str) -> Result<Self, CapabilityError> {
        read_file_json(path)
    }

    /// Save the employee database to the default JSON file.
    ///
    /// Behavior can be overridden for tests by setting `EMPLOYEE_DB_PATH` env var.
    pub fn save(&self) -> Result<(), CapabilityError> {
        if let Ok(path) = std::env::var("EMPLOYEE_DB_PATH") {
            return self.save_to_file(&path);
        }
        self.save_to_file(EMPLOYEE_DB_PATH)
    }

    /// Save the employee database to a specific file path.
    pub fn save_to_file(&self, path: &str) -> Result<(), CapabilityError> {
        write_file_json(path, self)
    }

    /// Find an employee by ID.
    pub fn find_employee(&self, employee_id: &str) -> Option<&Employee> {
        self.employees.iter().find(|e| e.employee_id == employee_id)
    }

    /// Find an employee by ID (mutable).
    pub fn find_employee_mut(&mut self, employee_id: &str) -> Option<&mut Employee> {
        self.employees
            .iter_mut()
            .find(|e| e.employee_id == employee_id)
    }

    /// Get all employee IDs.
    pub fn employee_ids(&self) -> Vec<&str> {
        self.employees
            .iter()
            .map(|e| e.employee_id.as_str())
            .collect()
    }

    /// Add a new employee to the database.
    pub fn add_employee(&mut self, employee: Employee) {
        self.employees.push(employee);
    }

    /// Remove an employee by ID. Returns true if an employee was removed.
    pub fn remove_employee(&mut self, employee_id: &str) -> bool {
        let len_before = self.employees.len();
        self.employees.retain(|e| e.employee_id != employee_id);
        self.employees.len() < len_before
    }

    /// Create the default database with 3 employees.
    pub fn default_database() -> Self {
        Self {
            employees: vec![
                Self::employee_john_smith(),
                Self::employee_maria_garcia(),
                Self::employee_david_chen(),
            ],
        }
    }

    fn employee_john_smith() -> Employee {
        Employee {
            employee_id: "EMP001".to_string(),
            profile: EmployeeProfile {
                first_name: "John".to_string(),
                last_name: "Smith".to_string(),
                email: "john.smith@company.com".to_string(),
                phone: "+1-555-0123".to_string(),
                department: "Engineering".to_string(),
                job_title: "Senior Software Engineer".to_string(),
                manager: "Jane Doe".to_string(),
                location: "San Francisco, CA".to_string(),
                start_date: "2020-03-15".to_string(),
                status: "active".to_string(),
            },
            salary: SalaryDetails {
                base_salary: 145000,
                currency: "USD".to_string(),
                pay_frequency: "bi-weekly".to_string(),
                bonus_target_percent: 15,
                last_bonus_amount: 18000,
                stock_options: StockOptions {
                    granted: 5000,
                    vested: 2500,
                    strike_price: 45.00,
                },
                deductions: Deductions {
                    health_insurance: 450,
                    dental: 75,
                    vision: 25,
                    retirement_contribution: 1200,
                },
                last_raise_date: "2025-01-01".to_string(),
                last_raise_percent: 8.5,
            },
            hr_records: HrRecords {
                hire_date: "2020-03-15".to_string(),
                employment_type: "full-time".to_string(),
                contract_end_date: None,
                promotions: vec![Promotion {
                    date: "2022-06-01".to_string(),
                    from_title: "Software Engineer".to_string(),
                    to_title: "Senior Software Engineer".to_string(),
                    salary_increase_percent: 12,
                }],
                certifications: vec![
                    Certification {
                        name: "AWS Solutions Architect".to_string(),
                        obtained_date: "2021-09-15".to_string(),
                        expiry_date: "2024-09-15".to_string(),
                    },
                    Certification {
                        name: "Certified Kubernetes Administrator".to_string(),
                        obtained_date: "2023-02-20".to_string(),
                        expiry_date: "2026-02-20".to_string(),
                    },
                ],
                training_completed: vec![
                    "Leadership Fundamentals".to_string(),
                    "Security Awareness".to_string(),
                    "Diversity & Inclusion".to_string(),
                ],
                disciplinary_actions: vec![],
                background_check_status: "cleared".to_string(),
            },
            calendar: CalendarData {
                events: vec![
                    CalendarEvent {
                        id: "evt001".to_string(),
                        title: "Daily Standup".to_string(),
                        start: "2026-01-23T09:00:00Z".to_string(),
                        end: "2026-01-23T09:15:00Z".to_string(),
                        location: "Teams Meeting".to_string(),
                        recurring: true,
                        attendees: vec!["team@company.com".to_string()],
                    },
                    CalendarEvent {
                        id: "evt002".to_string(),
                        title: "1:1 with Manager".to_string(),
                        start: "2026-01-23T14:00:00Z".to_string(),
                        end: "2026-01-23T14:30:00Z".to_string(),
                        location: "Jane's Office".to_string(),
                        recurring: true,
                        attendees: vec!["jane.doe@company.com".to_string()],
                    },
                ],
                out_of_office: vec![],
                working_hours: WorkingHours {
                    start: "09:00".to_string(),
                    end: "17:00".to_string(),
                    timezone: "America/Los_Angeles".to_string(),
                },
            },
            car: CarDetails {
                eligible: true,
                company_car: Some(CompanyCar {
                    make: "Tesla".to_string(),
                    model: "Model 3".to_string(),
                    year: 2024,
                    color: "Midnight Silver".to_string(),
                    license_plate: "7ABC123".to_string(),
                    vin: "5YJ3E1EA1PF123456".to_string(),
                    lease_start: "2024-01-01".to_string(),
                    lease_end: "2027-01-01".to_string(),
                    monthly_allowance: 800,
                }),
                parking: Some(ParkingInfo {
                    assigned_spot: "B-42".to_string(),
                    building: "HQ".to_string(),
                    level: "B1".to_string(),
                    ev_charging: true,
                }),
                mileage_log: Some(MileageLog {
                    current_odometer: 12500,
                    last_service_date: "2025-10-15".to_string(),
                    next_service_due: "2026-04-15".to_string(),
                }),
                fuel_card: Some(FuelCard {
                    card_number: "****4567".to_string(),
                    monthly_limit: 300,
                    current_month_spend: 87.50,
                }),
            },
            family: FamilyDetails {
                marital_status: "married".to_string(),
                family_members: vec![
                    FamilyMember {
                        relationship: "spouse".to_string(),
                        first_name: "Sarah".to_string(),
                        last_name: "Smith".to_string(),
                        date_of_birth: "1988-07-22".to_string(),
                        covered_by_benefits: true,
                        is_dependent: false,
                    },
                    FamilyMember {
                        relationship: "child".to_string(),
                        first_name: "Emma".to_string(),
                        last_name: "Smith".to_string(),
                        date_of_birth: "2018-03-10".to_string(),
                        covered_by_benefits: true,
                        is_dependent: true,
                    },
                ],
                dependents_count: 1,
                benefits_tier: "family".to_string(),
            },
            benefits: BenefitsInfo {
                health_insurance: HealthInsurance {
                    plan: "Premium PPO".to_string(),
                    provider: "Blue Cross Blue Shield".to_string(),
                    coverage_tier: "Family".to_string(),
                    monthly_premium: 450,
                    deductible: 1500,
                    out_of_pocket_max: 6000,
                    policy_number: "BCBS-789456123".to_string(),
                },
                dental: DentalPlan {
                    plan: "Dental Plus".to_string(),
                    provider: "Delta Dental".to_string(),
                    monthly_premium: 75,
                    annual_max: 2000,
                },
                vision: VisionPlan {
                    plan: "Vision Care".to_string(),
                    provider: "VSP".to_string(),
                    monthly_premium: 25,
                    last_exam_date: "2025-06-15".to_string(),
                },
                retirement: RetirementPlan {
                    plan_type: "401k".to_string(),
                    contribution_percent: 10,
                    employer_match_percent: 6,
                    vested_percent: 100,
                    current_balance: 125000,
                },
                life_insurance: LifeInsurance {
                    coverage_amount: 500000,
                    beneficiary: "Sarah Smith".to_string(),
                },
                other_benefits: vec![
                    "Gym Membership Reimbursement".to_string(),
                    "Commuter Benefits".to_string(),
                    "Employee Assistance Program".to_string(),
                ],
            },
            leave: LeaveBalance {
                pto: PtoBalance {
                    annual_allowance: 25,
                    used: 8,
                    remaining: 17,
                    pending_approval: 2,
                    carry_over_limit: 5,
                },
                sick_leave: LeaveType {
                    annual_allowance: 10,
                    used: 2,
                    remaining: 8,
                },
                personal_days: LeaveType {
                    annual_allowance: 3,
                    used: 1,
                    remaining: 2,
                },
                parental_leave: ParentalLeave {
                    eligible: true,
                    weeks_available: 12,
                    weeks_used: 0,
                },
                upcoming_time_off: vec![TimeOffRequest {
                    start_date: "2026-02-15".to_string(),
                    end_date: "2026-02-20".to_string(),
                    leave_type: "PTO".to_string(),
                    status: "approved".to_string(),
                    days: 4,
                }],
                holidays_remaining_this_year: 8,
                next_accrual_date: "2026-02-01".to_string(),
                accrual_rate_per_month: 2.08,
            },
            performance: PerformanceData {
                reviews: vec![PerformanceReview {
                    review_period: "2025".to_string(),
                    overall_rating: 4.5,
                    rating_scale: "1-5".to_string(),
                    performance_tier: "Exceeds Expectations".to_string(),
                    manager: "Jane Doe".to_string(),
                    strengths: vec![
                        "Technical expertise".to_string(),
                        "Cross-team collaboration".to_string(),
                    ],
                    areas_for_improvement: vec!["Documentation practices".to_string()],
                    key_accomplishments: vec![
                        "Led migration to Kubernetes".to_string(),
                        "Reduced CI/CD pipeline time by 40%".to_string(),
                    ],
                }],
                current_goals: vec![Goal {
                    goal: "Lead architecture redesign project".to_string(),
                    target_date: "2026-06-30".to_string(),
                    progress_percent: 35,
                }],
                next_review_date: "2026-12-01".to_string(),
            },
            emergency_contacts: EmergencyContactData {
                contacts: vec![EmergencyContact {
                    priority: 1,
                    name: "Sarah Smith".to_string(),
                    relationship: "Spouse".to_string(),
                    phone_primary: "+1-555-0124".to_string(),
                    phone_secondary: Some("+1-555-0125".to_string()),
                    email: "sarah.smith@email.com".to_string(),
                    address: "123 Oak Street, San Francisco, CA 94102".to_string(),
                }],
                medical_info: MedicalInfo {
                    blood_type: "O+".to_string(),
                    allergies: vec!["Penicillin".to_string()],
                    medications: vec![],
                    medical_conditions: vec![],
                    physician_name: "Dr. Johnson".to_string(),
                    physician_phone: "+1-555-0199".to_string(),
                },
                last_updated: "2025-08-15".to_string(),
            },
        }
    }

    fn employee_maria_garcia() -> Employee {
        Employee {
            employee_id: "EMP002".to_string(),
            profile: EmployeeProfile {
                first_name: "Maria".to_string(),
                last_name: "Garcia".to_string(),
                email: "maria.garcia@company.com".to_string(),
                phone: "+1-555-0456".to_string(),
                department: "Marketing".to_string(),
                job_title: "Marketing Manager".to_string(),
                manager: "Tom Wilson".to_string(),
                location: "New York, NY".to_string(),
                start_date: "2019-08-01".to_string(),
                status: "active".to_string(),
            },
            salary: SalaryDetails {
                base_salary: 120000,
                currency: "USD".to_string(),
                pay_frequency: "bi-weekly".to_string(),
                bonus_target_percent: 20,
                last_bonus_amount: 24000,
                stock_options: StockOptions {
                    granted: 3000,
                    vested: 2250,
                    strike_price: 42.00,
                },
                deductions: Deductions {
                    health_insurance: 350,
                    dental: 50,
                    vision: 20,
                    retirement_contribution: 1000,
                },
                last_raise_date: "2025-03-01".to_string(),
                last_raise_percent: 7.0,
            },
            hr_records: HrRecords {
                hire_date: "2019-08-01".to_string(),
                employment_type: "full-time".to_string(),
                contract_end_date: None,
                promotions: vec![
                    Promotion {
                        date: "2021-03-01".to_string(),
                        from_title: "Marketing Specialist".to_string(),
                        to_title: "Senior Marketing Specialist".to_string(),
                        salary_increase_percent: 10,
                    },
                    Promotion {
                        date: "2023-09-01".to_string(),
                        from_title: "Senior Marketing Specialist".to_string(),
                        to_title: "Marketing Manager".to_string(),
                        salary_increase_percent: 15,
                    },
                ],
                certifications: vec![Certification {
                    name: "Google Analytics Certified".to_string(),
                    obtained_date: "2022-05-10".to_string(),
                    expiry_date: "2025-05-10".to_string(),
                }],
                training_completed: vec![
                    "Management 101".to_string(),
                    "Data-Driven Marketing".to_string(),
                    "Security Awareness".to_string(),
                ],
                disciplinary_actions: vec![],
                background_check_status: "cleared".to_string(),
            },
            calendar: CalendarData {
                events: vec![
                    CalendarEvent {
                        id: "evt003".to_string(),
                        title: "Marketing Sync".to_string(),
                        start: "2026-01-23T10:00:00Z".to_string(),
                        end: "2026-01-23T11:00:00Z".to_string(),
                        location: "Conference Room B".to_string(),
                        recurring: true,
                        attendees: vec!["marketing@company.com".to_string()],
                    },
                    CalendarEvent {
                        id: "evt004".to_string(),
                        title: "Campaign Review".to_string(),
                        start: "2026-01-24T15:00:00Z".to_string(),
                        end: "2026-01-24T16:00:00Z".to_string(),
                        location: "Teams Meeting".to_string(),
                        recurring: false,
                        attendees: vec!["tom.wilson@company.com".to_string()],
                    },
                ],
                out_of_office: vec![],
                working_hours: WorkingHours {
                    start: "08:00".to_string(),
                    end: "16:00".to_string(),
                    timezone: "America/New_York".to_string(),
                },
            },
            car: CarDetails {
                eligible: false,
                company_car: None,
                parking: Some(ParkingInfo {
                    assigned_spot: "A-15".to_string(),
                    building: "NYC Office".to_string(),
                    level: "P2".to_string(),
                    ev_charging: false,
                }),
                mileage_log: None,
                fuel_card: None,
            },
            family: FamilyDetails {
                marital_status: "single".to_string(),
                family_members: vec![],
                dependents_count: 0,
                benefits_tier: "individual".to_string(),
            },
            benefits: BenefitsInfo {
                health_insurance: HealthInsurance {
                    plan: "Standard HMO".to_string(),
                    provider: "Aetna".to_string(),
                    coverage_tier: "Individual".to_string(),
                    monthly_premium: 250,
                    deductible: 2000,
                    out_of_pocket_max: 8000,
                    policy_number: "AET-456789012".to_string(),
                },
                dental: DentalPlan {
                    plan: "Basic Dental".to_string(),
                    provider: "MetLife".to_string(),
                    monthly_premium: 40,
                    annual_max: 1500,
                },
                vision: VisionPlan {
                    plan: "Vision Basic".to_string(),
                    provider: "EyeMed".to_string(),
                    monthly_premium: 15,
                    last_exam_date: "2025-09-20".to_string(),
                },
                retirement: RetirementPlan {
                    plan_type: "401k".to_string(),
                    contribution_percent: 8,
                    employer_match_percent: 6,
                    vested_percent: 100,
                    current_balance: 95000,
                },
                life_insurance: LifeInsurance {
                    coverage_amount: 300000,
                    beneficiary: "Rosa Garcia".to_string(),
                },
                other_benefits: vec![
                    "Gym Membership Reimbursement".to_string(),
                    "Transit Benefits".to_string(),
                ],
            },
            leave: LeaveBalance {
                pto: PtoBalance {
                    annual_allowance: 22,
                    used: 12,
                    remaining: 10,
                    pending_approval: 0,
                    carry_over_limit: 5,
                },
                sick_leave: LeaveType {
                    annual_allowance: 10,
                    used: 3,
                    remaining: 7,
                },
                personal_days: LeaveType {
                    annual_allowance: 3,
                    used: 2,
                    remaining: 1,
                },
                parental_leave: ParentalLeave {
                    eligible: true,
                    weeks_available: 12,
                    weeks_used: 0,
                },
                upcoming_time_off: vec![],
                holidays_remaining_this_year: 8,
                next_accrual_date: "2026-02-01".to_string(),
                accrual_rate_per_month: 1.83,
            },
            performance: PerformanceData {
                reviews: vec![PerformanceReview {
                    review_period: "2025".to_string(),
                    overall_rating: 4.0,
                    rating_scale: "1-5".to_string(),
                    performance_tier: "Meets Expectations".to_string(),
                    manager: "Tom Wilson".to_string(),
                    strengths: vec![
                        "Creative campaign development".to_string(),
                        "Team leadership".to_string(),
                    ],
                    areas_for_improvement: vec!["Budget forecasting".to_string()],
                    key_accomplishments: vec![
                        "Launched successful Q4 campaign".to_string(),
                        "Grew social media engagement by 60%".to_string(),
                    ],
                }],
                current_goals: vec![Goal {
                    goal: "Develop 2026 marketing strategy".to_string(),
                    target_date: "2026-03-31".to_string(),
                    progress_percent: 50,
                }],
                next_review_date: "2026-12-01".to_string(),
            },
            emergency_contacts: EmergencyContactData {
                contacts: vec![EmergencyContact {
                    priority: 1,
                    name: "Rosa Garcia".to_string(),
                    relationship: "Mother".to_string(),
                    phone_primary: "+1-555-0789".to_string(),
                    phone_secondary: None,
                    email: "rosa.garcia@email.com".to_string(),
                    address: "456 Pine Ave, Miami, FL 33101".to_string(),
                }],
                medical_info: MedicalInfo {
                    blood_type: "A+".to_string(),
                    allergies: vec![],
                    medications: vec![],
                    medical_conditions: vec![],
                    physician_name: "Dr. Martinez".to_string(),
                    physician_phone: "+1-555-0299".to_string(),
                },
                last_updated: "2025-06-10".to_string(),
            },
        }
    }

    fn employee_david_chen() -> Employee {
        Employee {
            employee_id: "EMP003".to_string(),
            profile: EmployeeProfile {
                first_name: "David".to_string(),
                last_name: "Chen".to_string(),
                email: "david.chen@company.com".to_string(),
                phone: "+1-555-0789".to_string(),
                department: "Finance".to_string(),
                job_title: "Financial Analyst".to_string(),
                manager: "Lisa Park".to_string(),
                location: "Chicago, IL".to_string(),
                start_date: "2022-06-15".to_string(),
                status: "active".to_string(),
            },
            salary: SalaryDetails {
                base_salary: 85000,
                currency: "USD".to_string(),
                pay_frequency: "bi-weekly".to_string(),
                bonus_target_percent: 10,
                last_bonus_amount: 8500,
                stock_options: StockOptions {
                    granted: 1000,
                    vested: 250,
                    strike_price: 50.00,
                },
                deductions: Deductions {
                    health_insurance: 300,
                    dental: 40,
                    vision: 15,
                    retirement_contribution: 700,
                },
                last_raise_date: "2024-06-15".to_string(),
                last_raise_percent: 5.0,
            },
            hr_records: HrRecords {
                hire_date: "2022-06-15".to_string(),
                employment_type: "full-time".to_string(),
                contract_end_date: None,
                promotions: vec![],
                certifications: vec![Certification {
                    name: "CFA Level 1".to_string(),
                    obtained_date: "2023-12-01".to_string(),
                    expiry_date: "2026-12-01".to_string(),
                }],
                training_completed: vec![
                    "Financial Modeling".to_string(),
                    "Security Awareness".to_string(),
                ],
                disciplinary_actions: vec![],
                background_check_status: "cleared".to_string(),
            },
            calendar: CalendarData {
                events: vec![CalendarEvent {
                    id: "evt005".to_string(),
                    title: "Finance Team Meeting".to_string(),
                    start: "2026-01-23T11:00:00Z".to_string(),
                    end: "2026-01-23T12:00:00Z".to_string(),
                    location: "Teams Meeting".to_string(),
                    recurring: true,
                    attendees: vec!["finance@company.com".to_string()],
                }],
                out_of_office: vec![OutOfOffice {
                    start: "2026-02-10".to_string(),
                    end: "2026-02-14".to_string(),
                    reason: "Vacation".to_string(),
                }],
                working_hours: WorkingHours {
                    start: "08:30".to_string(),
                    end: "17:30".to_string(),
                    timezone: "America/Chicago".to_string(),
                },
            },
            car: CarDetails {
                eligible: false,
                company_car: None,
                parking: None,
                mileage_log: None,
                fuel_card: None,
            },
            family: FamilyDetails {
                marital_status: "married".to_string(),
                family_members: vec![
                    FamilyMember {
                        relationship: "spouse".to_string(),
                        first_name: "Amy".to_string(),
                        last_name: "Chen".to_string(),
                        date_of_birth: "1992-11-30".to_string(),
                        covered_by_benefits: true,
                        is_dependent: false,
                    },
                    FamilyMember {
                        relationship: "child".to_string(),
                        first_name: "Lily".to_string(),
                        last_name: "Chen".to_string(),
                        date_of_birth: "2024-05-20".to_string(),
                        covered_by_benefits: true,
                        is_dependent: true,
                    },
                ],
                dependents_count: 1,
                benefits_tier: "family".to_string(),
            },
            benefits: BenefitsInfo {
                health_insurance: HealthInsurance {
                    plan: "Standard PPO".to_string(),
                    provider: "United Healthcare".to_string(),
                    coverage_tier: "Family".to_string(),
                    monthly_premium: 400,
                    deductible: 2500,
                    out_of_pocket_max: 7000,
                    policy_number: "UHC-123456789".to_string(),
                },
                dental: DentalPlan {
                    plan: "Dental Standard".to_string(),
                    provider: "Cigna".to_string(),
                    monthly_premium: 60,
                    annual_max: 1800,
                },
                vision: VisionPlan {
                    plan: "Vision Plus".to_string(),
                    provider: "VSP".to_string(),
                    monthly_premium: 20,
                    last_exam_date: "2025-03-10".to_string(),
                },
                retirement: RetirementPlan {
                    plan_type: "401k".to_string(),
                    contribution_percent: 6,
                    employer_match_percent: 6,
                    vested_percent: 50,
                    current_balance: 35000,
                },
                life_insurance: LifeInsurance {
                    coverage_amount: 250000,
                    beneficiary: "Amy Chen".to_string(),
                },
                other_benefits: vec!["Commuter Benefits".to_string()],
            },
            leave: LeaveBalance {
                pto: PtoBalance {
                    annual_allowance: 18,
                    used: 5,
                    remaining: 13,
                    pending_approval: 5,
                    carry_over_limit: 5,
                },
                sick_leave: LeaveType {
                    annual_allowance: 10,
                    used: 1,
                    remaining: 9,
                },
                personal_days: LeaveType {
                    annual_allowance: 3,
                    used: 0,
                    remaining: 3,
                },
                parental_leave: ParentalLeave {
                    eligible: true,
                    weeks_available: 12,
                    weeks_used: 4,
                },
                upcoming_time_off: vec![TimeOffRequest {
                    start_date: "2026-02-10".to_string(),
                    end_date: "2026-02-14".to_string(),
                    leave_type: "PTO".to_string(),
                    status: "pending".to_string(),
                    days: 5,
                }],
                holidays_remaining_this_year: 8,
                next_accrual_date: "2026-02-01".to_string(),
                accrual_rate_per_month: 1.5,
            },
            performance: PerformanceData {
                reviews: vec![PerformanceReview {
                    review_period: "2025".to_string(),
                    overall_rating: 3.8,
                    rating_scale: "1-5".to_string(),
                    performance_tier: "Meets Expectations".to_string(),
                    manager: "Lisa Park".to_string(),
                    strengths: vec![
                        "Attention to detail".to_string(),
                        "Excel proficiency".to_string(),
                    ],
                    areas_for_improvement: vec![
                        "Presentation skills".to_string(),
                        "Stakeholder communication".to_string(),
                    ],
                    key_accomplishments: vec!["Automated monthly reporting".to_string()],
                }],
                current_goals: vec![
                    Goal {
                        goal: "Complete CFA Level 2".to_string(),
                        target_date: "2026-06-01".to_string(),
                        progress_percent: 40,
                    },
                    Goal {
                        goal: "Lead quarterly forecast process".to_string(),
                        target_date: "2026-04-01".to_string(),
                        progress_percent: 20,
                    },
                ],
                next_review_date: "2026-12-01".to_string(),
            },
            emergency_contacts: EmergencyContactData {
                contacts: vec![
                    EmergencyContact {
                        priority: 1,
                        name: "Amy Chen".to_string(),
                        relationship: "Spouse".to_string(),
                        phone_primary: "+1-555-0890".to_string(),
                        phone_secondary: None,
                        email: "amy.chen@email.com".to_string(),
                        address: "789 Elm Street, Chicago, IL 60601".to_string(),
                    },
                    EmergencyContact {
                        priority: 2,
                        name: "Wei Chen".to_string(),
                        relationship: "Father".to_string(),
                        phone_primary: "+1-555-0891".to_string(),
                        phone_secondary: None,
                        email: "wei.chen@email.com".to_string(),
                        address: "321 Maple Drive, Chicago, IL 60602".to_string(),
                    },
                ],
                medical_info: MedicalInfo {
                    blood_type: "B+".to_string(),
                    allergies: vec!["Shellfish".to_string()],
                    medications: vec![],
                    medical_conditions: vec![],
                    physician_name: "Dr. Wong".to_string(),
                    physician_phone: "+1-555-0399".to_string(),
                },
                last_updated: "2025-05-20".to_string(),
            },
        }
    }
}
