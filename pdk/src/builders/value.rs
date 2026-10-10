//! A number where a [`Value`] is expected.

use stormlight_mod_abi::math::Value;

/// What a builder accepts wherever the schema wants a [`Value`]: a bare number for
/// a constant, or any expression already built. A plain `18.0` says what
/// `Value::Const(18.0)` says.
pub trait IntoValue {
    /// The expression this stands for.
    fn into_value(self) -> Value;
}

impl IntoValue for Value {
    fn into_value(self) -> Value {
        self
    }
}

impl IntoValue for f32 {
    fn into_value(self) -> Value {
        Value::Const(self)
    }
}
