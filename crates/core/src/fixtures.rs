//! Synthetic demo policies. These are examples, not production risk models.
use crate::*;

fn metric(name: &str, unit: &str, initial: i64, min: i64, max: i64) -> Metric {
    Metric {
        name: name.into(),
        unit: unit.into(),
        initial,
        min,
        max,
    }
}
fn term(metric: u32, target: i64, shape: Shape, weight: u32) -> Term {
    Term {
        metric,
        target,
        shape,
        weight,
    }
}
fn recipe(
    name: &str,
    actor: &str,
    n: usize,
    effects: &[(usize, i64)],
    complete: &[(usize, i64)],
    requires: Vec<(u32, u32)>,
) -> Recipe {
    let mut reserve = vec![0; n];
    let mut finish = vec![0; n];
    for (i, v) in effects {
        reserve[*i] = *v
    }
    for (i, v) in complete {
        finish[*i] = *v
    }
    Recipe {
        name: name.into(),
        actor: format!("{actor}.test.near"),
        executor: "executor.test.near".into(),
        reserve,
        complete: finish,
        max_units: 20,
        requires,
    }
}
pub const NAMES: [&str; 6] = ["station", "gpu", "friday", "inventory", "hiring", "risk"];
pub fn config(name: &str) -> Result<Config, Error> {
    let mut c = Config {
        name: name.into(),
        owner: "owner.test.near".into(),
        metrics: vec![],
        terms: vec![],
        recipes: vec![],
        max_age_seconds: 0,
    };
    match name {
        "station" => {
            // Each window supplies 12 MW. Four MW are unavailable to docking:
            // the 8 MW hard booking ceiling protects life support.
            c = config("gpu")?;
            c.name = name.into();
            for (i, ship) in ["Atlas", "Lyra", "Vega"].iter().enumerate() {
                c.metrics[i].name = format!("{ship} service allocation");
                for window in 0..2 {
                    let r = &mut c.recipes[i * 2 + window];
                    r.name = format!("{ship}: docking window {}", window + 1);
                    r.actor = format!("{}.test.near", ship.to_lowercase());
                }
            }
            for m in &mut c.metrics {
                m.unit = "MW".into();
            }
            for i in 3..5 {
                c.metrics[i].name = format!("Window {} docking power", i - 2);
                c.metrics[i].max = 8;
            }
        }
        "gpu" | "friday" => {
            let friday = name == "friday";
            let unit = if friday {
                "production lots"
            } else {
                "GPU blocks"
            };
            c.metrics = vec![
                metric("Research / customer A coverage", unit, 0, 0, 20),
                metric("Evaluation / customer B coverage", unit, 0, 0, 20),
                metric("Data / customer C coverage", unit, 0, 0, 20),
                metric(
                    if friday {
                        "Friday machine bookings"
                    } else {
                        "Window 1 bookings"
                    },
                    unit,
                    0,
                    0,
                    12,
                ),
                metric(
                    if friday {
                        "Tuesday machine bookings"
                    } else {
                        "Window 2 bookings"
                    },
                    unit,
                    0,
                    0,
                    12,
                ),
            ];
            if friday {
                c.metrics
                    .push(metric("Friday dispatch slots", "slots", 0, 0, 12));
                c.metrics
                    .push(metric("Tuesday dispatch slots", "slots", 0, 0, 12));
            }
            for i in 0..3 {
                c.terms.push(term(i, 4, Shape::Target, 1))
            }
            for i in 3..5 {
                c.terms.push(term(i, 8, Shape::Excess, 10))
            }
            let n = c.metrics.len();
            for (i, a) in ["research", "evaluation", "data"].iter().enumerate() {
                for s in 0..2 {
                    let mut effects = vec![(i, 1), (3 + s, 1)];
                    if friday {
                        effects.push((5 + s, 1));
                    }
                    c.recipes.push(recipe(
                        &format!(
                            "{a}: {}",
                            if s == 0 {
                                if friday {
                                    "Friday"
                                } else {
                                    "now"
                                }
                            } else if friday {
                                "Tuesday"
                            } else {
                                "next window"
                            }
                        ),
                        a,
                        n,
                        &effects,
                        &[],
                        vec![],
                    ));
                }
            }
        }
        "inventory" => {
            c.metrics = vec![
                metric("Depot position", "parts", 6, 4, 20),
                metric("Branch A position", "parts", 6, 0, 20),
                metric("Branch B position", "parts", 0, 0, 20),
                metric("Depot available", "parts", 6, 4, 20),
                metric("Branch A available", "parts", 6, 0, 20),
                metric("Branch B available", "parts", 0, 0, 20),
                metric("Reserved / in transit", "parts", 0, 0, 20),
            ];
            for i in 0..3 {
                c.terms.push(term(i, 4, Shape::Target, 1));
            }
            c.recipes = vec![
                recipe(
                    "Depot to A",
                    "branch-a",
                    7,
                    &[(0, -1), (1, 1), (3, -1), (6, 1)],
                    &[(4, 1), (6, -1)],
                    vec![],
                ),
                recipe(
                    "Depot to B",
                    "branch-b",
                    7,
                    &[(0, -1), (2, 1), (3, -1), (6, 1)],
                    &[(5, 1), (6, -1)],
                    vec![],
                ),
                recipe(
                    "A shares surplus with B",
                    "branch-a",
                    7,
                    &[(1, -1), (2, 1), (4, -1), (6, 1)],
                    &[(5, 1), (6, -1)],
                    vec![],
                ),
            ];
        }
        "hiring" => {
            c.metrics = vec![
                metric("Research funded coverage", "credits", 0, 0, 10),
                metric("Implementation funded coverage", "credits", 0, 0, 10),
                metric("Review funded coverage", "credits", 0, 0, 10),
                metric("Unencumbered discretionary funds", "credits", 5, 0, 10),
                metric("Review earmark", "credits", 2, 0, 2),
                metric("Active escrow", "credits", 0, 0, 7),
                metric("Paid lifetime", "credits", 0, 0, 7),
                metric("Protected reserve", "credits", 3, 3, 3),
            ];
            for (i, t) in [2, 3, 2].iter().enumerate() {
                c.terms.push(term(i as u32, *t, Shape::Target, 1));
            }
            c.recipes = vec![
                recipe(
                    "Research worker units",
                    "research",
                    8,
                    &[(0, 1), (3, -1), (5, 1)],
                    &[(5, -1), (6, 1)],
                    vec![],
                ),
                recipe(
                    "Implementation worker units",
                    "implementation",
                    8,
                    &[(1, 1), (3, -1), (5, 1)],
                    &[(5, -1), (6, 1)],
                    vec![],
                ),
                recipe(
                    "Independent review units",
                    "review",
                    8,
                    &[(2, 1), (4, -1), (5, 1)],
                    &[(5, -1), (6, 1)],
                    vec![(0, 2), (1, 3)],
                ),
            ];
        }
        "risk" => {
            c.max_age_seconds = 300;
            c.metrics = vec![
                metric("Selloff stress loss", "$100k", 60, 0, 100),
                metric("Funding stress loss", "$100k", 40, 0, 80),
                metric("Volatility stress loss", "$100k", 30, 0, 60),
                metric("Free collateral above reserve", "$100k", 40, 0, 40),
            ];
            for (i, t) in [80, 60, 40].iter().enumerate() {
                c.terms.push(term(i as u32, *t, Shape::Excess, 1));
            }
            c.recipes = vec![
                recipe(
                    "Credit position: half-size lots",
                    "credit",
                    4,
                    &[(0, 20), (1, 5), (3, -5)],
                    &[],
                    vec![],
                ),
                recipe(
                    "Equity position",
                    "equity",
                    4,
                    &[(0, 10), (2, 5), (3, -5)],
                    &[],
                    vec![],
                ),
                recipe(
                    "Macro position",
                    "macro",
                    4,
                    &[(1, 10), (2, 5), (3, -5)],
                    &[],
                    vec![],
                ),
                recipe(
                    "Selloff hedge (credit only after fill)",
                    "macro",
                    4,
                    &[(3, -2)],
                    &[(0, -10)],
                    vec![],
                ),
            ];
        }
        _ => return Err(Error::InvalidConfig),
    }
    c.validate()?;
    Ok(c)
}
/// (recipe index, requested units); each request is re-previewed at current version.
pub fn trace(name: &str) -> Vec<(u32, u32)> {
    match name {
        "station" => vec![(0, 4), (2, 2), (4, 4), (5, 4), (3, 2)],
        "gpu" | "friday" => vec![(0, 4), (2, 2), (4, 4), (4, 2), (5, 2), (3, 2)],
        "inventory" => vec![(0, 2), (1, 2), (2, 2)],
        "hiring" => vec![(0, 5), (0, 2), (1, 3), (2, 2)],
        "risk" => vec![(0, 2), (0, 1), (1, 1), (3, 1), (1, 1)],
        _ => vec![],
    }
}
