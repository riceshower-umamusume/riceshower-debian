use anyhow::{bail, Context, Result};
use std::cmp::Ordering;

pub fn compare(left: &str, right: &str) -> Result<Ordering> {
    Ok(parse(left)?.cmp(&parse(right)?))
}

pub fn parse(value: &str) -> Result<Vec<u64>> {
    if value.is_empty() {
        bail!("version must not be empty");
    }
    let mut parts = Vec::new();
    for part in value.split('.') {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            bail!("invalid numeric version '{value}'");
        }
        parts.push(
            part.parse::<u64>()
                .with_context(|| format!("invalid numeric version '{value}'"))?,
        );
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numeric_versions_by_component() {
        assert_eq!(compare("1.2.3", "1.2.3").unwrap(), Ordering::Equal);
        assert_eq!(compare("0.20.1", "0.21.0").unwrap(), Ordering::Less);
        assert_eq!(compare("1.10.0", "1.9.9").unwrap(), Ordering::Greater);
        assert_eq!(compare("1.0", "1.0.0").unwrap(), Ordering::Less);
    }

    #[test]
    fn rejects_non_numeric_constraints() {
        assert!(parse("").is_err());
        assert!(parse("1..2").is_err());
        assert!(parse("1.2rc1").is_err());
        assert!(parse("1.2.3-1").is_err());
    }
}
