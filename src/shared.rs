pub struct StickPos {
    pub x: i16,
    pub y: i16,
}

#[derive(Default)]
pub struct InputState {
    pub left:  Option<StickPos>,
    pub right: Option<StickPos>,
    pub menu:  bool,
}
