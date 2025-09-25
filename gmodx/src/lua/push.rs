use crate::lua;

pub trait Push {
    fn push(self, l: lua::State);
}

impl Push for i8 {
    fn push(self, l: lua::State) {
        l.direct_push_number(self as lua::Number);
    }
}

impl Push for i16 {
    fn push(self, l: lua::State) {
        l.direct_push_number(self as lua::Number);
    }
}

impl Push for i32 {
    fn push(self, l: lua::State) {
        l.direct_push_number(self as lua::Number);
    }
}

impl Push for i64 {
    fn push(self, l: lua::State) {
        if (lua::MIN_SAFE_INTEGER..=lua::MAX_SAFE_INTEGER).contains(&self) {
            l.direct_push_number(self as lua::Number);
        } else {
            l.push_string(&self.to_string());
        }
    }
}

impl Push for i128 {
    fn push(self, l: lua::State) {
        if self >= lua::MIN_SAFE_INTEGER as i128 && self <= lua::MAX_SAFE_INTEGER as i128 {
            l.direct_push_number(self as lua::Number);
        } else {
            l.push_string(&self.to_string());
        }
    }
}

impl Push for isize {
    fn push(self, l: lua::State) {
        if self >= lua::MIN_SAFE_INTEGER as isize && self <= lua::MAX_SAFE_INTEGER as isize {
            l.direct_push_number(self as lua::Number);
        } else {
            l.push_string(&self.to_string());
        }
    }
}

impl Push for u8 {
    fn push(self, l: lua::State) {
        l.direct_push_number(self as lua::Number);
    }
}

impl Push for u16 {
    fn push(self, l: lua::State) {
        l.direct_push_number(self as lua::Number);
    }
}

impl Push for u32 {
    fn push(self, l: lua::State) {
        l.direct_push_number(self as lua::Number);
    }
}

impl Push for u64 {
    fn push(self, l: lua::State) {
        if self <= lua::MAX_SAFE_INTEGER as u64 {
            l.direct_push_number(self as lua::Number);
        } else {
            l.push_string(&self.to_string());
        }
    }
}

impl Push for u128 {
    fn push(self, l: lua::State) {
        if self <= lua::MAX_SAFE_INTEGER as u128 {
            l.direct_push_number(self as lua::Number);
        } else {
            l.push_string(&self.to_string());
        }
    }
}

impl Push for usize {
    fn push(self, l: lua::State) {
        if self <= lua::MAX_SAFE_INTEGER as usize {
            l.direct_push_number(self as lua::Number);
        } else {
            l.push_string(&self.to_string());
        }
    }
}

impl Push for f32 {
    fn push(self, l: lua::State) {
        l.direct_push_number(self as lua::Number);
    }
}

impl Push for f64 {
    fn push(self, l: lua::State) {
        l.direct_push_number(self as lua::Number);
    }
}

impl Push for bool {
    fn push(self, l: lua::State) {
        l.push_bool(self);
    }
}

impl Push for &str {
    fn push(self, l: lua::State) {
        l.push_string(self);
    }
}

impl Push for String {
    #[inline]
    fn push(self, l: lua::State) {
        l.push_string(&self);
    }
}

impl lua::State {
    #[inline]
    pub fn push<T: Push>(self, value: T) {
        value.push(self)
    }
}
