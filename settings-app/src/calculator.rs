//! Bounded calculator terms. Property identifiers are data, never executable text.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Token {
    Number { value: String },
    Aggregate {
        property: String,
        operation: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        calculation: Option<Vec<Token>>,
    },
    Property { property: String },
    Count,
    Operator { value: String },
    Function { value: String },
    Open,
    Close,
    Comma,
}

#[derive(Debug)]
enum Expression {
    Number(f64),
    Aggregate(String, String, Option<Box<Expression>>),
    Property(String),
    Count,
    Negate(Box<Expression>),
    Binary(String, Box<Expression>, Box<Expression>),
    Root(Box<Expression>, Box<Expression>),
}

pub struct Calculator(Expression);
fn canonical_operation(operation: &str) -> Option<&'static str> {
    match operation.trim().to_ascii_lowercase().as_str() {
        "total" | "sum" => Some("total"),
        "average" | "avg" => Some("average"),
        "median" => Some("median"),
        "minimum" | "min" => Some("minimum"),
        "maximum" | "max" => Some("maximum"),
        _ => None,
    }
}
impl Calculator {
    pub fn parse(tokens: &[Token]) -> Result<Self, String> {
        Self::parse_scoped(tokens, false, 0, &mut 0)
    }

    fn parse_scoped(
        tokens: &[Token],
        allow_property: bool,
        depth: u8,
        total_terms: &mut usize,
    ) -> Result<Self, String> {
        if depth > 48 {
            return Err("This calculation is nested too deeply.".into());
        }
        if tokens.is_empty() {
            return Err("Enter a calculation.".into());
        }
        *total_terms = (*total_terms).saturating_add(tokens.len());
        if *total_terms > 256 {
            return Err("Use at most 256 calculator terms.".into());
        }
        for token in tokens {
            match token {
                Token::Number { value } if value.len() > 32 => {
                    return Err("This number is too long.".into())
                }
                Token::Aggregate {
                    property,
                    operation,
                    calculation,
                } if property.len() > 256
                    || property.is_empty()
                    || canonical_operation(operation).is_none()
                    || calculation
                        .as_ref()
                        .is_some_and(|tokens| tokens.is_empty()) =>
                {
                    return Err("Choose a supported numeric property and aggregation.".into())
                }
                Token::Property { property } if property.is_empty() || property.len() > 256 => {
                    return Err("Choose a supported property.".into())
                }
                _ => {}
            }
        }
        let mut parser = Parser {
            tokens,
            cursor: 0,
            allow_property,
            total_terms,
        };
        let expression = parser.expression(0, depth)?;
        if parser.cursor != tokens.len() {
            return Err("Add an operator between terms, or check the parentheses.".into());
        }
        Ok(Self(expression))
    }

    pub fn properties(&self) -> Vec<String> {
        fn visit(node: &Expression, output: &mut Vec<String>) {
            match node {
                Expression::Aggregate(property, _, calculation) => {
                    if !output.contains(property) {
                        output.push(property.clone());
                    }
                    if let Some(calculation) = calculation {
                        visit(calculation, output);
                    }
                }
                Expression::Property(property) => {
                    if !output.contains(property) {
                        output.push(property.clone());
                    }
                }
                Expression::Negate(inner) => visit(inner, output),
                Expression::Binary(_, left, right) | Expression::Root(left, right) => {
                    visit(left, output);
                    visit(right, output);
                }
                _ => {}
            }
        }
        let mut result = Vec::new();
        visit(&self.0, &mut result);
        result
    }

    #[cfg(test)]
    pub fn evaluate(&self, cards: &[Value]) -> Result<Option<f64>, String> {
        self.evaluate_with_boolean_properties(cards, &BTreeSet::new())
    }

    /// Evaluate the expression using the source schema's boolean property keys.
    /// Boolean aggregates always produce 0 or 1, while numeric aggregates keep
    /// their existing unrestricted numeric behavior.
    pub fn evaluate_with_boolean_properties(
        &self,
        cards: &[Value],
        boolean_properties: &BTreeSet<String>,
    ) -> Result<Option<f64>, String> {
        fn finite(value: f64) -> Result<Option<f64>, String> {
            if value.is_finite() {
                Ok(Some(if value == 0.0 { 0.0 } else { value }))
            } else {
                Err("The result is outside the supported numeric range.".into())
            }
        }
        fn run(
            node: &Expression,
            cards: &[Value],
            boolean_properties: &BTreeSet<String>,
        ) -> Result<Option<f64>, String> {
            match node {
                Expression::Number(value) => finite(*value),
                Expression::Count => finite(cards.len() as f64),
                Expression::Property(property) => {
                    let Some(card) = cards.first() else {
                        return Ok(None);
                    };
                    let value = card
                        .get("properties")
                        .and_then(|p| p.get(property))
                        .unwrap_or(&Value::Null);
                    let is_boolean = boolean_properties.contains(property);
                    if value.is_null() && !is_boolean {
                        return Ok(None);
                    }
                    let numeric = if is_boolean {
                        match value {
                            Value::Null => 0.0,
                            Value::Bool(value) => {
                                if *value { 1.0 } else { 0.0 }
                            }
                            Value::Number(_) => value
                                .as_f64()
                                .filter(|n| n.is_finite())
                                .ok_or("A boolean property contains an incompatible value.")?
                                .clamp(0.0, 1.0),
                            _ => {
                                return Err(
                                    "A boolean property contains an incompatible value.".into(),
                                )
                            }
                        }
                    } else {
                        value
                            .as_f64()
                            .filter(|n| n.is_finite())
                            .ok_or("A numeric property contains an incompatible value.")?
                    };
                    finite(numeric)
                }
                Expression::Aggregate(property, operation, calculation) => {
                    let is_boolean = boolean_properties.contains(property);
                    let mut values = Vec::new();
                    for card in cards {
                        if let Some(calculation) = calculation {
                            if let Some(value) = run(
                                calculation,
                                std::slice::from_ref(card),
                                boolean_properties,
                            )? {
                                values.push(value);
                            }
                            continue;
                        }
                        let value = card
                            .get("properties")
                            .and_then(|p| p.get(property))
                            .unwrap_or(&Value::Null);
                        if value.is_null() && !is_boolean {
                            continue;
                        }
                        let value = if is_boolean {
                            match value {
                                Value::Null | Value::Bool(false) => 0.0,
                                Value::Bool(true) => 1.0,
                                Value::Number(_) => value
                                    .as_f64()
                                    .filter(|n| n.is_finite())
                                    .ok_or("A boolean property contains an incompatible value.")?
                                    .clamp(0.0, 1.0),
                                _ => {
                                    return Err(
                                        "A boolean property contains an incompatible value.".into(),
                                    )
                                }
                            }
                        } else {
                            value
                                .as_f64()
                                .filter(|n| n.is_finite())
                                .ok_or("A numeric property contains an incompatible value.")?
                        };
                        values.push(value);
                    }
                    if values.is_empty() {
                        return Ok(
                            if (is_boolean && calculation.is_none()) || operation == "total" {
                                Some(0.0)
                            } else {
                                None
                            },
                        );
                    }
                    let value = match operation.as_str() {
                        "minimum" => values.iter().copied().fold(f64::INFINITY, f64::min),
                        "maximum" => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                        "median" => {
                            values.sort_by(f64::total_cmp);
                            let middle = values.len() / 2;
                            if values.len() % 2 == 1 {
                                values[middle]
                            } else {
                                values[middle - 1] / 2.0 + values[middle] / 2.0
                            }
                        }
                        "average" => {
                            let count = values.len() as f64;
                            if is_boolean {
                                values.iter().sum::<f64>() / count
                            } else {
                                values.iter().map(|value| value / count).sum()
                            }
                        }
                        _ => values.iter().sum(),
                    };
                    if is_boolean && calculation.is_none() {
                        finite(value.clamp(0.0, 1.0).round())
                    } else {
                        finite(value)
                    }
                }
                Expression::Negate(value) => match run(value, cards, boolean_properties)? {
                    Some(value) => finite(-value),
                    None => Ok(None),
                },
                Expression::Binary(operator, left, right) => {
                    let (Some(left), Some(right)) = (
                        run(left, cards, boolean_properties)?,
                        run(right, cards, boolean_properties)?,
                    ) else {
                        return Ok(None);
                    };
                    match operator.as_str() {
                        "+" => finite(left + right), "-" => finite(left - right), "*" => finite(left * right),
                        "/" if right == 0.0 => Err("Cannot divide by zero.".into()),
                        "/" => finite(left / right),
                        "^" if left == 0.0 && right == 0.0 => Err("Zero to the power of zero is undefined.".into()),
                        "^" if left < 0.0 && right.fract() != 0.0 => Err("This power has no real-number result. Use an odd root for a negative value.".into()),
                        "^" => finite(left.powf(right)),
                        _ => Err("Unsupported operator.".into()),
                    }
                }
                Expression::Root(value, degree) => {
                    let (Some(value), Some(degree)) = (
                        run(value, cards, boolean_properties)?,
                        run(degree, cards, boolean_properties)?,
                    )
                    else {
                        return Ok(None);
                    };
                    if degree.fract() != 0.0 || !(1.0..=1024.0).contains(&degree) {
                        return Err("Root degrees must be whole numbers from 1 to 1024.".into());
                    }
                    if value < 0.0 && degree % 2.0 == 0.0 {
                        return Err(
                            "An even root of a negative value has no real-number result.".into(),
                        );
                    }
                    finite(value.abs().powf(1.0 / degree) * if value < 0.0 { -1.0 } else { 1.0 })
                }
            }
        }
        run(&self.0, cards, boolean_properties)
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    cursor: usize,
    allow_property: bool,
    total_terms: &'a mut usize,
}
impl Parser<'_> {
    fn expect(&mut self, token: Token) -> Result<(), String> {
        if self.tokens.get(self.cursor) != Some(&token) {
            return Err("Check the parentheses and root arguments.".into());
        }
        self.cursor += 1;
        Ok(())
    }
    fn expression(&mut self, minimum: u8, depth: u8) -> Result<Expression, String> {
        if depth > 48 {
            return Err("This calculation is nested too deeply.".into());
        }
        let token = self
            .tokens
            .get(self.cursor)
            .ok_or("Complete the calculation.")?;
        self.cursor += 1;
        let mut left = match token {
            Token::Number { value } => Expression::Number(
                value
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .ok_or("Enter a finite number.")?,
            ),
            Token::Aggregate {
                property,
                operation,
                calculation,
            } => Expression::Aggregate(
                property.clone(),
                canonical_operation(operation)
                    .expect("aggregation was validated before parsing")
                    .to_string(),
                if let Some(calculation) = calculation {
                    Some(Box::new(Calculator::parse_scoped(
                        calculation,
                        true,
                        depth + 1,
                        self.total_terms,
                    )?.0))
                } else {
                    None
                },
            ),
            Token::Property { property } if self.allow_property => {
                Expression::Property(property.clone())
            }
            Token::Count => Expression::Count,
            Token::Operator { value } if value == "-" || value == "+" => {
                let expression = self.expression(20, depth + 1)?;
                if value == "-" {
                    Expression::Negate(Box::new(expression))
                } else {
                    expression
                }
            }
            Token::Open => {
                let expression = self.expression(0, depth + 1)?;
                self.expect(Token::Close)?;
                expression
            }
            Token::Function { value } if value == "sqrt" || value == "root" => {
                self.expect(Token::Open)?;
                let expression = self.expression(0, depth + 1)?;
                let degree = if value == "sqrt" {
                    Expression::Number(2.0)
                } else {
                    self.expect(Token::Comma)?;
                    self.expression(0, depth + 1)?
                };
                self.expect(Token::Close)?;
                Expression::Root(Box::new(expression), Box::new(degree))
            }
            _ => return Err("Choose a number, property, or root.".into()),
        };
        while let Some(Token::Operator { value }) = self.tokens.get(self.cursor) {
            let precedence = match value.as_str() {
                "+" | "-" => 10,
                "*" | "/" => 20,
                "^" => 30,
                _ => return Err("Unsupported operator.".into()),
            };
            if precedence < minimum {
                break;
            }
            self.cursor += 1;
            let right = self.expression(precedence + u8::from(value != "^"), depth + 1)?;
            left = Expression::Binary(value.clone(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }
}

pub fn format(value: Option<f64>) -> String {
    let Some(value) = value else {
        return "No value".into();
    };
    if value == 0.0 {
        return "0".into();
    }
    if value.abs() >= 1e10 || value.abs() < 0.000001 {
        return format!("{value:.6e}");
    }
    format!("{value:.6}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn evaluate(tokens: Value, cards: Vec<Value>) -> Result<Option<f64>, String> {
        Calculator::parse(&serde_json::from_value::<Vec<Token>>(tokens).unwrap())?.evaluate(&cards)
    }
    #[test]
    fn precedence_roots_domains_and_aggregations() {
        let n = |v: &str| json!({"kind":"number","value":v});
        let op = |v: &str| json!({"kind":"operator","value":v});
        assert_eq!(
            evaluate(json!([n("2"), op("+"), n("3"), op("*"), n("4")]), vec![]).unwrap(),
            Some(14.0)
        );
        assert_eq!(
            evaluate(json!([n("2"), op("^"), n("3"), op("^"), n("2")]), vec![]).unwrap(),
            Some(512.0)
        );
        assert_eq!(
            evaluate(json!([op("-"), n("2"), op("^"), n("2")]), vec![]).unwrap(),
            Some(-4.0)
        );
        assert_eq!(evaluate(json!([{"kind":"function","value":"root"},{"kind":"open"},n("-8"),{"kind":"comma"},n("3"),{"kind":"close"}]), vec![]).unwrap(), Some(-2.0));
        assert_eq!(
            evaluate(json!([n("10"), op("-"), n("4"), op("/"), n("2")]), vec![]).unwrap(),
            Some(8.0)
        );
        assert_eq!(
            evaluate(
                json!([{"kind":"function","value":"sqrt"},{"kind":"open"},n("9"),{"kind":"close"}]),
                vec![]
            )
            .unwrap(),
            Some(3.0)
        );
        assert_eq!(
            evaluate(json!([{"kind":"count"}]), vec![json!({}), json!({})]).unwrap(),
            Some(2.0)
        );
        for degree in ["0", "-1", "1.5", "1025"] {
            assert!(evaluate(json!([{"kind":"function","value":"root"},{"kind":"open"},n("8"),{"kind":"comma"},n(degree),{"kind":"close"}]),vec![]).is_err());
        }
        assert!(Calculator::parse(&vec![Token::Count; 257]).is_err());
        assert!(evaluate(json!([n("1e999")]), vec![]).is_err());
        for tokens in [
            json!([n("1"), op("/"), n("0")]),
            json!([n("0"), op("^"), n("0")]),
            json!([n("-2"), op("^"), n("0.5")]),
            json!([{"kind":"function","value":"sqrt"},{"kind":"open"},n("-1"),{"kind":"close"}]),
            json!([n("1"), op("+")]),
        ] {
            assert!(evaluate(tokens, vec![]).is_err());
        }
        for (operation, expected) in [
            ("total", Some(8.0)),
            ("average", Some(4.0)),
            ("median", Some(4.0)),
            ("minimum", Some(3.0)),
            ("maximum", Some(5.0)),
        ] {
            let tokens = json!([{"kind":"aggregate","property":"estimate","operation":operation}]);
            assert_eq!(
                evaluate(
                    tokens.clone(),
                    vec![
                        json!({"properties":{"estimate":3}}),
                        json!({"properties":{"estimate":5}}),
                        json!({"properties":{}})
                    ]
                )
                .unwrap(),
                expected
            );
            assert_eq!(
                evaluate(tokens.clone(), vec![]).unwrap(),
                if operation == "total" {
                    Some(0.0)
                } else {
                    None
                }
            );
            assert!(evaluate(tokens, vec![json!({"properties":{"estimate":"bad"}})]).is_err());
        }
        for (alias, canonical, expected) in [
            ("sum", "total", Some(8.0)),
            ("avg", "average", Some(4.0)),
            ("min", "minimum", Some(3.0)),
            ("max", "maximum", Some(5.0)),
        ] {
            let tokens = json!([{"kind":"aggregate","property":"estimate","operation":alias}]);
            assert_eq!(evaluate(tokens, vec![json!({"properties":{"estimate":3}}), json!({"properties":{"estimate":5}})]).unwrap(), expected);
            assert_eq!(canonical_operation(alias), Some(canonical));
        }
        let median = json!([{"kind":"aggregate","property":"estimate","operation":"median"}]);
        assert_eq!(
            evaluate(
                median.clone(),
                vec![
                    json!({"properties":{"estimate":9}}),
                    json!({"properties":{"estimate":1}}),
                    json!({"properties":{"estimate":7}}),
                ],
            )
            .unwrap(),
            Some(7.0)
        );
        assert_eq!(
            evaluate(
                median.clone(),
                vec![
                    json!({"properties":{"estimate":null}}),
                    json!({"properties":{"estimate":1}}),
                    json!({"properties":{"estimate":8}}),
                ],
            )
            .unwrap(),
            Some(4.5)
        );
        assert_eq!(
            evaluate(
                median,
                vec![
                    json!({"properties":{"estimate":f64::MAX}}),
                    json!({"properties":{"estimate":f64::MAX}}),
                ],
            )
            .unwrap(),
            Some(f64::MAX)
        );
        assert_eq!(format(Some(3.5)), "3.5");
        assert_eq!(format(Some(-0.0)), "0");
    }

    #[test]
    fn boolean_aggregates_are_normalized_to_binary_results() {
        let boolean_keys = BTreeSet::from(["done".to_string()]);
        let aggregate = |operation: &str| {
            Calculator::parse(&[Token::Aggregate {
                property: "done".into(),
                operation: operation.into(),
                calculation: None,
            }])
            .unwrap()
        };
        let evaluate = |operation: &str, values: Vec<Value>| {
            aggregate(operation).evaluate_with_boolean_properties(
                &values
                    .into_iter()
                    .map(|value| json!({"properties":{"done":value}}))
                    .collect::<Vec<_>>(),
                &boolean_keys,
            )
        };

        assert_eq!(evaluate("total", vec![json!(true); 20]).unwrap(), Some(1.0));
        assert_eq!(
            evaluate("average", vec![json!(true), json!(false)]).unwrap(),
            Some(1.0)
        );
        assert_eq!(
            evaluate("average", vec![json!(0.4), json!(0.4), json!(0.7)]).unwrap(),
            Some(1.0)
        );
        assert_eq!(evaluate("average", vec![json!(0.49)]).unwrap(), Some(0.0));
        assert_eq!(evaluate("average", vec![json!(0.5)]).unwrap(), Some(1.0));
        assert_eq!(evaluate("average", vec![json!(0.51)]).unwrap(), Some(1.0));
        assert_eq!(evaluate("total", vec![json!(-8.0)]).unwrap(), Some(0.0));
        assert_eq!(evaluate("total", vec![json!(9.0)]).unwrap(), Some(1.0));
        assert_eq!(
            evaluate("minimum", vec![json!(true), json!(false)]).unwrap(),
            Some(0.0)
        );
        assert_eq!(
            evaluate("maximum", vec![json!(true), json!(false)]).unwrap(),
            Some(1.0)
        );
        for operation in ["total", "average", "median", "minimum", "maximum"] {
            assert_eq!(evaluate(operation, vec![]).unwrap(), Some(0.0));
        }
        assert!(evaluate("total", vec![json!("true")]).is_err());
        assert!(evaluate("total", vec![json!({"value":true})]).is_err());

        let numeric = aggregate("total");
        assert!(numeric
            .evaluate(&[json!({"properties":{"done":true}})])
            .is_err());
        assert_eq!(
            Calculator::parse(&[Token::Aggregate {
                property: "amount".into(),
                operation: "total".into(),
                calculation: None,
            }])
            .unwrap()
            .evaluate(&[json!({"properties":{"amount":4}})])
            .unwrap(),
            Some(4.0)
        );
    }

    #[test]
    fn per_card_calculations_transform_before_aggregation() {
        let tokens = json!([{
            "kind":"aggregate",
            "property":"price",
            "operation":"sum",
            "calculation":[
                {"kind":"property","property":"price"},
                {"kind":"operator","value":"*"},
                {"kind":"property","property":"quantity"}
            ]
        }]);
        let calculator = Calculator::parse(&serde_json::from_value::<Vec<Token>>(tokens).unwrap())
            .unwrap();
        assert_eq!(
            calculator.properties(),
            vec!["price".to_string(), "quantity".to_string()]
        );
        assert_eq!(
            calculator
                .evaluate(&[
                    json!({"properties":{"price":10,"quantity":2}}),
                    json!({"properties":{"price":3,"quantity":4}}),
                ])
                .unwrap(),
            Some(32.0)
        );

        // An explicit aggregate inside the calculation sees only its current card.
        let nested = vec![Token::Aggregate {
            property: "price".into(),
            operation: "total".into(),
            calculation: Some(vec![
                Token::Property {
                    property: "price".into(),
                },
                Token::Operator { value: "+".into() },
                Token::Aggregate {
                    property: "tax".into(),
                    operation: "total".into(),
                    calculation: None,
                },
            ]),
        }];
        let calculator = Calculator::parse(&nested).unwrap();
        assert_eq!(
            calculator.properties(),
            vec!["price".to_string(), "tax".to_string()]
        );
        assert_eq!(
            calculator
                .evaluate(&[
                    json!({"properties":{"price":10,"tax":1}}),
                    json!({"properties":{"price":20,"tax":2}}),
                ])
                .unwrap(),
            Some(33.0)
        );
    }

    #[test]
    fn per_card_missing_boolean_and_legacy_schema_semantics() {
        let boolean_keys = BTreeSet::from(["done".to_string()]);
        let calculator = Calculator::parse(&[Token::Aggregate {
            property: "done".into(),
            operation: "sum".into(),
            calculation: Some(vec![
                Token::Property {
                    property: "done".into(),
                },
                Token::Number { value: "10".into() },
                Token::Operator { value: "*".into() },
            ]),
        }]);
        // Incomplete formulas are rejected during parsing.
        assert!(calculator.is_err());
        let calculator = Calculator::parse(&[Token::Aggregate {
            property: "done".into(),
            operation: "sum".into(),
            calculation: Some(vec![
                Token::Property {
                    property: "done".into(),
                },
                Token::Operator { value: "*".into() },
                Token::Number { value: "10".into() },
            ]),
        }])
        .unwrap();
        assert_eq!(
            calculator
                .evaluate_with_boolean_properties(
                    &[
                        json!({"properties":{"done":true}}),
                        json!({"properties":{"done":false}}),
                        json!({"properties":{}}),
                    ],
                    &boolean_keys,
                )
                .unwrap(),
            Some(10.0)
        );
        assert!(Calculator::parse(&[Token::Property {
            property: "done".into(),
        }])
        .is_err());

        // Older stored tokens omit `calculation` and still deserialize and serialize
        // in their historical shape.
        let legacy: Token = serde_json::from_value(json!({
            "kind":"aggregate", "property":"price", "operation":"total"
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_value(legacy).unwrap(),
            json!({"kind":"aggregate", "property":"price", "operation":"total"})
        );
    }

    #[test]
    fn nested_calculation_respects_global_term_limit() {
        let formula = Token::Aggregate {
            property: "price".into(),
            operation: "total".into(),
            calculation: Some(vec![Token::Number { value: "1".into() }; 256]),
        };
        assert!(Calculator::parse(&[formula]).is_err());
    }
}
