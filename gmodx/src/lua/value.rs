use std::{
    collections::{VecDeque, vec_deque},
    fmt,
    ops::{Deref, DerefMut},
};

use crate::lua::{
    self, FromLuaMulti, Result, ToLuaMulti, ffi,
    traits::{FromLua, ToLua},
    value_ref::{ValueRef, ref_state},
};

#[derive(Clone, Debug)]
pub struct Value(pub(crate) ValueInner);

#[derive(Clone, Debug)]
pub(crate) enum ValueInner {
    Nil,
    Bool(bool),
    Number(f64),
    Ref(ValueRef),
}

// https://github.com/Facepunch/gmod-module-base/blob/development/include/GarrysMod/Lua/Types.h
#[repr(i32)]
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ValueKind {
    None = -1,
    Nil = 0,
    Bool,
    LightUserData,
    Number,
    String,
    Table,
    Function,
    UserData,
    Thread,

    // GMod Types
    Entity,
    Vector,
    Angle,
    PhysObj,
    Save,
    Restore,
    DamageInfo,
    EffectData,
    MoveData,
    RecipientFilter,
    UserCmd,
    ScriptedVehicle,
    Material,
    Panel,
    Particle,
    ParticleEmitter,
    Texture,
    UserMsg,
    ConVar,
    IMesh,
    Matrix,
    Sound,
    PixelVisHandle,
    DLight,
    Video,
    File,
    Locomotion,
    Path,
    NavArea,
    SoundHandle,
    NavLadder,
    ParticleSystem,
    ProjectedTexture,
    PhysCollide,
    SurfaceInfo,

    TypeCount,
}

impl ValueKind {
    #[must_use]
    #[inline]
    pub fn from_i32(id: i32) -> Self {
        assert!(
            id >= Self::None as i32 && id < Self::TypeCount as i32,
            "Invalid lua type id: {id}"
        );

        unsafe { std::mem::transmute(id) }
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Nil => "nil",
            Self::Bool => "boolean",
            Self::LightUserData => "lightuserdata",
            Self::Number => "number",
            Self::String => "string",
            Self::Table => "table",
            Self::Function => "function",
            Self::UserData => "userdata",
            Self::Thread => "thread",

            Self::Entity => "entity",
            Self::Vector => "vector",
            Self::Angle => "angle",
            Self::PhysObj => "physobj",
            Self::Save => "save",
            Self::Restore => "restore",
            Self::DamageInfo => "damageinfo",
            Self::EffectData => "effectdata",
            Self::MoveData => "movedata",
            Self::RecipientFilter => "recipientfilter",
            Self::UserCmd => "usercmd",
            Self::ScriptedVehicle => "scriptedvehicle",
            Self::Material => "material",
            Self::Panel => "panel",
            Self::Particle => "particle",
            Self::ParticleEmitter => "particleemitter",
            Self::Texture => "texture",
            Self::UserMsg => "usermsg",
            Self::ConVar => "convar",
            Self::IMesh => "imesh",
            Self::Matrix => "matrix",
            Self::Sound => "sound",
            Self::PixelVisHandle => "pixelvishandle",
            Self::DLight => "dlight",
            Self::Video => "video",
            Self::File => "file",
            Self::Locomotion => "locomotion",
            Self::Path => "path",
            Self::NavArea => "navarea",
            Self::SoundHandle => "soundhandle",
            Self::NavLadder => "navladder",
            Self::ParticleSystem => "particlesystem",
            Self::ProjectedTexture => "projectedtexture",
            Self::PhysCollide => "physcollide",
            Self::SurfaceInfo => "surfaceinfo",
            _ => "unknown",
        }
    }
}

impl fmt::Display for ValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Value {
    #[must_use]
    pub fn from_stack(l: &lua::State, index: i32) -> Self {
        ffi::lua_pushvalue(l.0, index);
        Self::pop_from_stack(l)
    }

    #[must_use]
    pub(crate) fn from_ref(r: ValueRef) -> Self {
        Self(ValueInner::Ref(r))
    }

    #[must_use]
    pub fn pop_from_stack(l: &lua::State) -> Self {
        let type_id = ffi::lua_type(l.0, -1);
        match ValueKind::from_i32(type_id) {
            ValueKind::Nil => {
                ffi::lua_pop(l.0, 1);
                Self(ValueInner::Nil)
            }
            ValueKind::Bool => {
                let val = ffi::lua_toboolean(l.0, -1);
                ffi::lua_pop(l.0, 1);
                Self(ValueInner::Bool(val))
            }
            ValueKind::Number => {
                let val = ffi::lua_tonumber(l.0, -1);
                ffi::lua_pop(l.0, 1);
                Self(ValueInner::Number(val))
            }
            _ => Self(ValueInner::Ref(ValueRef::pop_from(l, type_id))),
        }
    }

    #[must_use]
    pub fn type_id(&self) -> i32 {
        match &self.0 {
            ValueInner::Nil => ValueKind::Nil as i32,
            ValueInner::Bool(_) => ValueKind::Bool as i32,
            ValueInner::Number(_) => ValueKind::Number as i32,
            ValueInner::Ref(r) => r.type_id(),
        }
    }

    #[must_use]
    pub fn type_kind(&self) -> ValueKind {
        ValueKind::from_i32(self.type_id())
    }

    #[must_use]
    pub fn type_name(&self) -> &'static str {
        self.type_kind().as_str()
    }

    pub fn to<T: FromLua>(self, l: &lua::State) -> Result<T> {
        T::try_from_value(self, l)
    }

    pub fn push_to_stack(&self, l: &lua::State) {
        match &self.0 {
            ValueInner::Nil => ffi::lua_pushnil(l.0),
            ValueInner::Bool(b) => b.push_to_stack(l),
            ValueInner::Number(n) => n.push_to_stack(l),
            ValueInner::Ref(r) => r.push(l),
        }
    }

    pub(crate) fn index(&self) -> Option<i32> {
        match &self.0 {
            ValueInner::Ref(r) => Some(r.index()),
            _ => None,
        }
    }

    pub(crate) fn ref_state(&self) -> lua::State {
        ref_state()
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Value of type {} ({})", self.type_name(), self.type_id())
    }
}

impl ToLua for Value {
    fn push_to_stack(self, l: &lua::State) {
        Self::push_to_stack(&self, l);
    }

    fn to_value(self, _: &lua::State) -> Self {
        self
    }
}

impl ToLua for &Value {
    fn push_to_stack(self, l: &lua::State) {
        Value::push_to_stack(self, l);
    }

    fn to_value(self, _: &lua::State) -> Value {
        self.clone()
    }
}

impl FromLua for Value {
    fn try_from_stack(l: &lua::State, index: i32) -> Result<Self> {
        Ok(Self::from_stack(l, index))
    }

    fn try_from_value(value: Value, _: &lua::State) -> Result<Self> {
        Ok(value)
    }
}

#[derive(Default, Debug, Clone)]
pub struct MultiValue(VecDeque<Value>);

impl Deref for MultiValue {
    type Target = VecDeque<Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MultiValue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl MultiValue {
    /// Creates an empty `MultiValue` containing no values.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self(VecDeque::new())
    }

    /// Creates an empty `MultiValue` container with space for at least `capacity` elements.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(VecDeque::with_capacity(capacity))
    }

    /// Creates a `MultiValue` container from vector of values.
    ///
    /// This method works in *O*(1) time and does not allocate any additional memory.
    #[must_use]
    #[inline]
    pub fn from_vec(vec: Vec<Value>) -> Self {
        vec.into()
    }

    /// Consumes the `MultiValue` and returns a vector of values.
    ///
    /// This method needs *O*(*n*) data movement if the circular buffer doesn't happen to be at the
    /// beginning of the allocation.
    #[must_use]
    #[inline]
    pub fn into_vec(self) -> Vec<Value> {
        self.into()
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn from_lua_iter<T: ToLua>(
        l: &lua::State,
        iter: impl IntoIterator<Item = T>,
    ) -> Self {
        let iter = iter.into_iter();
        let mut multi_value = Self::with_capacity(iter.size_hint().0);
        for value in iter {
            multi_value.push_back(value.to_value(l));
        }
        multi_value
    }
}

impl From<Vec<Value>> for MultiValue {
    #[inline]
    fn from(value: Vec<Value>) -> Self {
        Self(value.into())
    }
}

impl From<MultiValue> for Vec<Value> {
    #[inline]
    fn from(value: MultiValue) -> Self {
        value.0.into()
    }
}

impl FromIterator<Value> for MultiValue {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Self {
        let mut multi_value = Self::new();
        multi_value.extend(iter);
        multi_value
    }
}

impl IntoIterator for MultiValue {
    type Item = Value;
    type IntoIter = vec_deque::IntoIter<Value>;

    #[inline]
    fn into_iter(mut self) -> Self::IntoIter {
        let deque = std::mem::take(&mut self.0);
        std::mem::forget(self);
        deque.into_iter()
    }
}

impl ToLuaMulti for MultiValue {
    fn push_to_stack_multi(self, l: &lua::State) {
        for value in self {
            value.push_to_stack(l);
        }
    }
}

impl ToLuaMulti for &MultiValue {
    fn push_to_stack_multi(self, l: &lua::State) {
        for value in self.iter() {
            value.push_to_stack(l);
        }
    }
}

impl FromLuaMulti for MultiValue {
    fn try_from_stack_multi(l: &lua::State, start_index: i32, count: i32) -> Result<(Self, i32)> {
        let mut multi_value = Self::with_capacity(count as usize);
        for i in 0..count {
            multi_value.push_back(Value::from_stack(l, start_index + i));
        }
        Ok((multi_value, count))
    }
}
