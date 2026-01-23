//! Get Benefits Info capability - returns employee benefits data from the database.

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

        let benefits = &employee.benefits;
        Ok(json!({
            "employee_id": employee.employee_id,
            "health_insurance": {
                "plan": benefits.health_insurance.plan,
                "provider": benefits.health_insurance.provider,
                "coverage_tier": benefits.health_insurance.coverage_tier,
                "monthly_premium": benefits.health_insurance.monthly_premium,
                "deductible": benefits.health_insurance.deductible,
                "out_of_pocket_max": benefits.health_insurance.out_of_pocket_max,
                "policy_number": benefits.health_insurance.policy_number
            },
            "dental": {
                "plan": benefits.dental.plan,
                "provider": benefits.dental.provider,
                "monthly_premium": benefits.dental.monthly_premium,
                "annual_max": benefits.dental.annual_max
            },
            "vision": {
                "plan": benefits.vision.plan,
                "provider": benefits.vision.provider,
                "monthly_premium": benefits.vision.monthly_premium,
                "last_exam_date": benefits.vision.last_exam_date
            },
            "retirement": {
                "plan_type": benefits.retirement.plan_type,
                "contribution_percent": benefits.retirement.contribution_percent,
                "employer_match_percent": benefits.retirement.employer_match_percent,
                "vested_percent": benefits.retirement.vested_percent,
                "current_balance": benefits.retirement.current_balance
            },
            "life_insurance": {
                "coverage_amount": benefits.life_insurance.coverage_amount,
                "beneficiary": benefits.life_insurance.beneficiary
            },
            "other_benefits": benefits.other_benefits
        }))
    });
}
