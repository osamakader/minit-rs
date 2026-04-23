use std::collections::{HashMap, HashSet, VecDeque};

use crate::service::ServiceConfig;

pub fn resolve_start_order(
    services: &[ServiceConfig],
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    if services.is_empty() {
        return Ok(Vec::new());
    }

    let mut provider_map: HashMap<&str, usize> = HashMap::new();
    for (idx, service) in services.iter().enumerate() {
        for provided in provided_tokens(service) {
            if let Some(previous_idx) = provider_map.insert(provided, idx) {
                return Err(format!(
                    "duplicate provider '{}' for services '{}' and '{}'",
                    provided, services[previous_idx].name, service.name
                )
                .into());
            }
        }
    }

    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); services.len()];
    let mut indegree: Vec<usize> = vec![0; services.len()];

    for (idx, service) in services.iter().enumerate() {
        for dep in &service.depends {
            let Some(&provider_idx) = provider_map.get(dep.as_str()) else {
                return Err(format!(
                    "service '{}' depends on '{}' but no provider exists",
                    service.name, dep
                )
                .into());
            };

            if provider_idx != idx {
                dependents[provider_idx].push(idx);
                indegree[idx] += 1;
            }
        }
    }

    let mut queue = VecDeque::new();
    for (idx, &degree) in indegree.iter().enumerate() {
        if degree == 0 {
            queue.push_back(idx);
        }
    }

    let mut order = Vec::with_capacity(services.len());
    while let Some(current) = queue.pop_front() {
        order.push(current);
        for &next in &dependents[current] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }

    if order.len() != services.len() {
        return Err("dependency cycle detected in services".into());
    }

    Ok(order)
}

fn provided_tokens(service: &ServiceConfig) -> Vec<&str> {
    let mut tokens = Vec::with_capacity(service.provides.len() + 1);
    let mut seen = HashSet::new();

    tokens.push(service.name.as_str());
    seen.insert(service.name.as_str());

    for item in &service.provides {
        if seen.insert(item.as_str()) {
            tokens.push(item.as_str());
        }
    }
    tokens
}
