use crate::lua;

pub trait IsNumber: lua::Push {}

impl IsNumber for i8 {}
impl IsNumber for i16 {}
impl IsNumber for i32 {}
impl IsNumber for i64 {}
impl IsNumber for i128 {}
impl IsNumber for isize {}
impl IsNumber for u8 {}
impl IsNumber for u16 {}
impl IsNumber for u32 {}
impl IsNumber for u64 {}
impl IsNumber for u128 {}
impl IsNumber for usize {}
impl IsNumber for f32 {}
impl IsNumber for f64 {}

impl lua::State {
    #[inline]
    pub fn push_number<N: IsNumber>(self, n: N) {
        n.push(self);
    }
}
