use bevy::prelude::*;

#[derive(Reflect, Debug, Copy, Clone, Default, PartialEq)]
pub struct Length(pub f32);

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Position3(pub Vec3);

impl Position3 {
    pub fn truncate(self) -> Position2 {
        Position2(self.0.truncate())
    }
}

#[derive(Reflect, Debug, Copy, Clone, Default, PartialEq)]
pub struct Position2(pub Vec2);

impl Position2 {
    pub fn transform(self, matrix: Mat3) -> Self {
        Position2(matrix.transform_point2(self.0))
    }
}
