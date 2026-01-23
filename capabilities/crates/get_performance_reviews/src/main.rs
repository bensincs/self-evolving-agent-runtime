//! Get Performance Reviews capability - returns performance review data from the database.

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

        let perf = &employee.performance;
        Ok(json!({
            "employee_id": employee.employee_id,
            "reviews": perf.reviews.iter().map(|r| json!({
                "review_period": r.review_period,
                "overall_rating": r.overall_rating,
                "rating_scale": r.rating_scale,
                "performance_tier": r.performance_tier,
                "manager": r.manager,
                "strengths": r.strengths,
                "areas_for_improvement": r.areas_for_improvement,
                "key_accomplishments": r.key_accomplishments
            })).collect::<Vec<_>>(),
            "current_goals": perf.current_goals.iter().map(|g| json!({
                "goal": g.goal,
                "target_date": g.target_date,
                "progress_percent": g.progress_percent
            })).collect::<Vec<_>>(),
            "next_review_date": perf.next_review_date
        }))
    });
}
