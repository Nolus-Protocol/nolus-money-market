use std::{array, iter, vec};

use serde_test::Token;

use either::Either;

use json_value::JsonValue;

enum SingleOrComposite {
    Single(Token),
    Composite(Vec<Token>),
}

impl IntoIterator for SingleOrComposite {
    type Item = Token;

    type IntoIter = Either<iter::Once<Token>, vec::IntoIter<Token>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Single(value) => Either::Left(iter::once(value)),
            Self::Composite(values) => Either::Right(values.into_iter()),
        }
    }
}

impl AsRef<[Token]> for SingleOrComposite {
    fn as_ref(&self) -> &[Token] {
        match self {
            Self::Single(token) => array::from_ref(token),
            Self::Composite(tokens) => tokens,
        }
    }
}

const fn null() -> SingleOrComposite {
    const { SingleOrComposite::Single(Token::None {}) }
}

const fn bool(value: bool) -> SingleOrComposite {
    SingleOrComposite::Single(Token::Bool(value))
}

const fn i8(value: i8) -> SingleOrComposite {
    SingleOrComposite::Single(Token::I8(value))
}

const fn u8(value: u8) -> SingleOrComposite {
    SingleOrComposite::Single(Token::U8(value))
}

const fn i16(value: i16) -> SingleOrComposite {
    SingleOrComposite::Single(Token::I16(value))
}

const fn u16(value: u16) -> SingleOrComposite {
    SingleOrComposite::Single(Token::U16(value))
}

const fn i32(value: i32) -> SingleOrComposite {
    SingleOrComposite::Single(Token::I32(value))
}

const fn u32(value: u32) -> SingleOrComposite {
    SingleOrComposite::Single(Token::U32(value))
}

const fn i64(value: i64) -> SingleOrComposite {
    SingleOrComposite::Single(Token::I64(value))
}

const fn u64(value: u64) -> SingleOrComposite {
    SingleOrComposite::Single(Token::U64(value))
}

const fn string(value: &'static str) -> SingleOrComposite {
    SingleOrComposite::Single(Token::String(value))
}

fn seq<T>(values: T) -> SingleOrComposite
where
    T: IntoIterator<Item = SingleOrComposite>,
{
    let mut v = vec![Token::Seq { len: None }];

    let mut groups_count = 0;

    for value in values {
        groups_count += 1;

        v.extend(value);
    }

    v[0] = Token::Seq {
        len: Some(groups_count),
    };

    v.push(Token::SeqEnd);

    SingleOrComposite::Composite(v)
}

fn map<T>(values: T) -> SingleOrComposite
where
    T: IntoIterator<Item = (SingleOrComposite, SingleOrComposite)>,
{
    let mut v = vec![Token::Map { len: None }];

    let mut groups_count = 0;

    for (key, value) in values {
        groups_count += 1;

        v.extend(key);

        v.extend(value);
    }

    v[0] = Token::Map {
        len: Some(groups_count),
    };

    v.push(Token::MapEnd);

    SingleOrComposite::Composite(v)
}

#[test]
fn test_null() {
    serde_test::assert_tokens(&JsonValue::Null, null().as_ref());
}

#[test]
fn test_boolean() {
    for value in [false, true] {
        serde_test::assert_tokens(&JsonValue::Bool(value), bool(value).as_ref());
    }
}

#[test]
fn test_signed_integer() {
    const I8: i8 = -123;

    const I16: i16 = -1234;

    const I32: i32 = -1234;

    const I64: i64 = -1234;

    serde_test::assert_de_tokens(&JsonValue::I64(I8.into()), i8(I8).as_ref());

    serde_test::assert_de_tokens(&JsonValue::I64(I16.into()), i16(I16).as_ref());

    serde_test::assert_de_tokens(&JsonValue::I64(I32.into()), i32(I32).as_ref());

    serde_test::assert_tokens(&JsonValue::I64(I64), i64(I64).as_ref());
}

#[test]
fn test_unsigned_integer() {
    const U8: u8 = 123;

    const U16: u16 = 1234;

    const U32: u32 = 1234;

    const U64: u64 = 1234;

    serde_test::assert_de_tokens(&JsonValue::U64(U8.into()), u8(U8).as_ref());

    serde_test::assert_de_tokens(&JsonValue::U64(U16.into()), u16(U16).as_ref());

    serde_test::assert_de_tokens(&JsonValue::U64(U32.into()), u32(U32).as_ref());

    serde_test::assert_tokens(&JsonValue::U64(U64), u64(U64).as_ref());
}

#[test]
fn test_string() {
    const VALUE: &'static str = "String";

    serde_test::assert_tokens(&JsonValue::String(VALUE.into()), string(VALUE).as_ref());
}

#[test]
fn test_empty_array() {
    serde_test::assert_tokens(&JsonValue::Array([].into()), seq([]).as_ref());
}

#[test]
fn test_flat_array() {
    serde_test::assert_tokens(
        &JsonValue::Array([JsonValue::Null].into()),
        &seq([null()]).as_ref(),
    );
}

#[test]
fn test_nested_array() {
    serde_test::assert_tokens(
        &JsonValue::Array(
            [
                JsonValue::Null,
                JsonValue::Array(
                    [
                        JsonValue::Null,
                        JsonValue::Array([].into()),
                        JsonValue::Bool(false),
                    ]
                    .into(),
                ),
            ]
            .into(),
        ),
        seq([null(), seq([null(), seq([]), bool(false)])]).as_ref(),
    );
}

#[test]
fn test_empty_object() {
    serde_test::assert_tokens(&JsonValue::Object([].into()), map([]).as_ref());
}

#[test]
fn test_flat_object() {
    const FIELD: &'static str = "null";

    serde_test::assert_tokens(
        &JsonValue::Object([(FIELD.to_string(), JsonValue::Null)].into()),
        map([(string(FIELD), null())]).as_ref(),
    );
}

#[test]
fn test_nested_object() {
    const NULL_FIELD: &str = "null";

    const ARRAY_FIELD: &str = "array";

    const ARRAY_WITH_OBJECTS_FIELD: &'static str = "array_with_objects";

    /// "Array of elements"/"Array Element 1"
    const AWO_AE1_VALUE: u64 = 1234;

    /// "Array of elements"/"Array Element 3"/"null"
    const AWO_AE3_NULL_FIELD: &str = "awo__3__null";

    /// "Array of elements"/"Array Element 3"/"i64"
    const AWO_AE3_I64_FIELD: &'static str = "awo__3__i64";

    /// "Array of elements"/"Array Element 3"/"i64"
    const AWO_AE3_I64_VALUE: i64 = -1234;

    serde_test::assert_tokens(
        &JsonValue::Object(
            [
                (NULL_FIELD.to_string(), JsonValue::Null),
                (ARRAY_FIELD.to_string(), JsonValue::Array([].into())),
                (
                    ARRAY_WITH_OBJECTS_FIELD.to_string(),
                    JsonValue::Array(
                        [
                            JsonValue::U64(AWO_AE1_VALUE),
                            JsonValue::Object([].into()),
                            JsonValue::Object(
                                [
                                    (AWO_AE3_NULL_FIELD.to_string(), JsonValue::Null),
                                    (
                                        AWO_AE3_I64_FIELD.to_string(),
                                        JsonValue::I64(AWO_AE3_I64_VALUE),
                                    ),
                                ]
                                .into(),
                            ),
                        ]
                        .into(),
                    ),
                ),
            ]
            .into(),
        ),
        map([
            (string(NULL_FIELD), null()),
            (string(ARRAY_FIELD), seq([])),
            (
                string(ARRAY_WITH_OBJECTS_FIELD),
                seq([
                    u64(AWO_AE1_VALUE),
                    map([]),
                    map([
                        (string(AWO_AE3_NULL_FIELD), null()),
                        (string(AWO_AE3_I64_FIELD), i64(AWO_AE3_I64_VALUE)),
                    ]),
                ]),
            ),
        ])
        .as_ref(),
    );
}
