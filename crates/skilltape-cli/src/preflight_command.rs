use std::path::PathBuf;
use std::process::ExitCode;

use skilltape_core::SkillPackage;
use skilltape_verify::policy_plan;

pub(crate) fn run(path: PathBuf, json: bool) -> ExitCode {
    let plan = SkillPackage::load(path)
        .ok()
        .and_then(|package| policy_plan(&package).ok());
    let Some(plan) = plan else {
        eprintln!("policy preflight failed: invalid package or preview capacity exceeded");
        return ExitCode::from(2);
    };
    if json {
        match serde_json::to_string(&plan) {
            Ok(document) => println!("{document}"),
            Err(_) => return ExitCode::from(2),
        }
    } else {
        println!(
            "Policy preflight: {} (execution: not-run)",
            if plan.allowed { "allowed" } else { "blocked" }
        );
        println!(
            "Package contract: {}",
            if plan.package_valid {
                "valid"
            } else {
                "invalid"
            }
        );
        for code in &plan.package_error_codes {
            println!("Package finding: {code}");
        }
        for check in &plan.checks {
            println!(
                "{} {}: {} — {}",
                check.ordinal, check.step_id, check.decision.code, check.decision.reason
            );
        }
        println!("Input data, sandbox availability and runtime results remain unverified.");
    }
    if plan.allowed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(3)
    }
}
