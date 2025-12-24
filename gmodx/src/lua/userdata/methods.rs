use std::ffi::CStr;

use crate::lua::{Function, function::IntoLuaFunction};

type Methods = Vec<(&'static CStr, Function)>;

#[derive(Default)]
pub struct MethodsBuilder(pub(crate) Methods);

impl MethodsBuilder {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    pub fn add<Marker>(
        &mut self,
        name: &'static CStr,
        func: impl IntoLuaFunction<Marker>,
    ) -> &mut Self {
        let callback = func.into_function();
        self.0.push((name, callback));
        self
    }
}
