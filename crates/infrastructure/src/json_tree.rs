use gedoens_domain::{Node, Scalar};
use serde_json::Value;

/// SOPS gibt jedes Format auf Wunsch als JSON aus. So brauchen wir nur einen Leser.
pub fn to_node(value: Value) -> Node {
    match value {
        Value::Null => Node::Leaf(Scalar::Null),
        Value::Bool(b) => Node::Leaf(Scalar::Bool(b)),
        Value::Number(n) => Node::Leaf(Scalar::Number(n.to_string())),
        Value::String(s) => Node::Leaf(Scalar::Text(s)),
        Value::Array(items) => Node::List(items.into_iter().map(to_node).collect()),
        Value::Object(entries) => Node::Map(entries.into_iter().map(|(k, v)| (k, to_node(v))).collect()),
    }
}

/// Ein Wert in der JSON-Schreibweise, die `sops set` erwartet.
pub fn to_json(scalar: &Scalar) -> String {
    match scalar {
        Scalar::Text(s) => Value::String(s.clone()).to_string(),
        Scalar::Number(n) => n.clone(),
        Scalar::Bool(b) => b.to_string(),
        Scalar::Null => "null".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order_and_types() {
        let node = to_node(serde_json::from_str(r#"{"b":1,"a":{"x":true,"y":null,"z":"t"}}"#).unwrap());
        let Node::Map(entries) = node else { panic!("kein Objekt") };
        assert_eq!(entries[0].0, "b");
        assert_eq!(entries[0].1, Node::Leaf(Scalar::Number("1".into())));
    }

    #[test]
    fn escapes_text_for_sops() {
        assert_eq!(to_json(&Scalar::Text("a\"b\nc".into())), r#""a\"b\nc""#);
        assert_eq!(to_json(&Scalar::Number("5432".into())), "5432");
    }
}
