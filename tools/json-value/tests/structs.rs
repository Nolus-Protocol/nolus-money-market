use serde::{Deserialize, Serialize};

use serde_json::{from_str as from_json_str, to_string as to_json_string};

use json_value::JsonValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct Struct {
    string_field: String,
    i32_field: i32,
}

#[test]
fn test_struct() {
    let original = Struct {
        string_field: "123".to_string(),
        i32_field: -456,
    };

    let ser_foo = to_json_string(&original).unwrap();

    assert_eq!(ser_foo, r#"{"string_field":"123","i32_field":-456}"#);

    let de_value: JsonValue = from_json_str(&ser_foo).unwrap();

    assert_eq!(
        de_value,
        JsonValue::Object(
            [
                (
                    "string_field".to_string(),
                    JsonValue::String("123".to_string()),
                ),
                ("i32_field".to_string(), JsonValue::I64(-456)),
            ]
            .into(),
        )
    );

    let ser_value = to_json_string(&de_value).unwrap();

    assert_eq!(ser_value, r#"{"string_field":"123","i32_field":-456}"#);

    let de_foo: Struct = from_json_str(&ser_value).unwrap();

    assert_eq!(original, de_foo);
}
