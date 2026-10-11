use galileo_core::Value;

pub fn translate(handler: &str) -> (&str, Option<Value>) {
    match handler {
        "offer-history-row" => ("offer-row", Some(Value::text(""))),
        "offer-settings-row" => ("offer-row", None),
        handler => (handler, None),
    }
}
