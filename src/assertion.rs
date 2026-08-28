use std::{cmp::Ordering, fmt::Write};

use num_bigint::BigInt;
use serde_json::Value;

use crate::error::AssertionError;

#[derive(Clone, Debug)]
pub struct Difference {
    pub path: String,
    pub expected: Option<Value>,
    pub actual: Option<Value>,
}

pub fn assert_json_equal(expected: &Value, actual: &Value) -> Result<(), AssertionError> {
    let mut differences = Vec::new();
    collect_differences(expected, actual, String::new(), &mut differences);
    let count = differences.len();
    if count == 0 {
        return Ok(());
    }
    differences.sort_by(|left, right| left.path.cmp(&right.path));
    let shown = differences.iter().take(20);
    let mut details = String::new();
    for difference in shown {
        let _ = writeln!(
            details,
            "  {}",
            if difference.path.is_empty() {
                "/"
            } else {
                &difference.path
            }
        );
        let _ = writeln!(
            details,
            "    expected: {}",
            format_value(difference.expected.as_ref())
        );
        let _ = writeln!(
            details,
            "      actual: {}",
            format_value(difference.actual.as_ref())
        );
    }
    if count > 20 {
        let _ = writeln!(details, "  ... {} more differences", count - 20);
    }
    Err(AssertionError {
        count,
        details: details.trim_end().into(),
    })
}

fn collect_differences(
    expected: &Value,
    actual: &Value,
    path: String,
    output: &mut Vec<Difference>,
) {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            let mut keys = expected.keys().chain(actual.keys()).collect::<Vec<_>>();
            keys.sort();
            keys.dedup();
            for key in keys {
                let child = format!("{path}/{}", escape_pointer(key));
                match (expected.get(key), actual.get(key)) {
                    (Some(expected), Some(actual)) => {
                        collect_differences(expected, actual, child, output)
                    }
                    (expected, actual) => output.push(Difference {
                        path: child,
                        expected: expected.cloned(),
                        actual: actual.cloned(),
                    }),
                }
            }
        }
        (Value::Array(expected), Value::Array(actual)) => {
            let max = expected.len().max(actual.len());
            for index in 0..max {
                let child = format!("{path}/{index}");
                match (expected.get(index), actual.get(index)) {
                    (Some(expected), Some(actual)) => {
                        collect_differences(expected, actual, child, output)
                    }
                    (expected, actual) => output.push(Difference {
                        path: child,
                        expected: expected.cloned(),
                        actual: actual.cloned(),
                    }),
                }
            }
        }
        (Value::Number(expected), Value::Number(actual)) if numbers_equal(expected, actual) => {}
        (expected, actual) if expected == actual => {}
        (expected, actual) => output.push(Difference {
            path,
            expected: Some(expected.clone()),
            actual: Some(actual.clone()),
        }),
    }
}

fn numbers_equal(left: &serde_json::Number, right: &serde_json::Number) -> bool {
    Decimal::parse(&left.to_string())
        .zip(Decimal::parse(&right.to_string()))
        .is_some_and(|(left, right)| left == right)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Decimal {
    coefficient: BigInt,
    scale: i64,
}

impl Decimal {
    fn parse(value: &str) -> Option<Self> {
        let (mantissa, exponent) = value
            .split_once(['e', 'E'])
            .map_or((value, 0), |(m, e)| (m, e.parse().ok().unwrap_or(0)));
        let (sign, mantissa) = if let Some(value) = mantissa.strip_prefix('-') {
            (-1, value)
        } else {
            (1, mantissa.strip_prefix('+').unwrap_or(mantissa))
        };
        let (whole, fraction) = mantissa
            .split_once('.')
            .map_or((mantissa, ""), |(w, f)| (w, f));
        if whole.is_empty() && fraction.is_empty()
            || !whole
                .bytes()
                .chain(fraction.bytes())
                .all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let digits = format!("{whole}{fraction}");
        let mut coefficient = digits.parse::<BigInt>().ok()? * sign;
        let mut scale = fraction.len() as i64 - exponent;
        if coefficient == BigInt::from(0) {
            return Some(Self {
                coefficient,
                scale: 0,
            });
        }
        while (&coefficient % 10u8) == BigInt::from(0) {
            coefficient /= 10u8;
            scale -= 1;
        }
        Some(Self { coefficient, scale })
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let scale = self.scale.max(other.scale);
        let left = &self.coefficient * BigInt::from(10u8).pow((scale - self.scale) as u32);
        let right = &other.coefficient * BigInt::from(10u8).pow((scale - other.scale) as u32);
        left.partial_cmp(&right)
    }
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn format_value(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "<missing>".into();
    };
    let text = serde_json::to_string(value).unwrap_or_else(|_| "<invalid JSON>".into());
    if text.len() <= 200 {
        text
    } else {
        let end = text
            .char_indices()
            .take_while(|(index, _)| *index < 197)
            .map(|(index, _)| index)
            .last()
            .unwrap_or(0);
        format!("{}… ({} bytes)", &text[..end], text.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compares_objects_without_key_order_and_numbers_without_notation() {
        assert_json_equal(&json!({"a": 1, "b": 1.0}), &json!({"b": 1, "a": 1})).unwrap();
    }

    #[test]
    fn escapes_json_pointer() {
        let error = assert_json_equal(&json!({"a/b~c": 1}), &json!({"a/b~c": 2})).unwrap_err();
        assert!(error.details.contains("/a~1b~0c"));
    }

    #[test]
    fn distinguishes_missing_null_and_limits_reported_differences() {
        let error = assert_json_equal(&json!({"value": null}), &json!({})).unwrap_err();
        assert!(error.details.contains("expected: null"));
        assert!(error.details.contains("actual: <missing>"));

        let expected = (0..21)
            .map(|index| (format!("key{index}"), json!(0)))
            .collect::<serde_json::Map<_, _>>();
        let actual = (0..21)
            .map(|index| (format!("key{index}"), json!(1)))
            .collect::<serde_json::Map<_, _>>();
        let error =
            assert_json_equal(&Value::Object(expected), &Value::Object(actual)).unwrap_err();
        assert_eq!(error.count, 21);
        assert!(error.details.contains("... 1 more differences"));
    }

    #[test]
    fn truncates_unicode_values_without_panicking() {
        let long = "あ".repeat(100);
        assert_json_equal(&json!(long), &json!("different")).unwrap_err();
    }
}
