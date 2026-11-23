//! What a key, the tray or the mini player can ask the player to do (the same commands as
//! runAction() in src/app/actions.ts).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    PlayPause,
    Next,
    Prev,
    SeekForward,
    SeekBack,
    VolumeUp,
    VolumeDown,
    Mute,
    Like,
    Shuffle,
    Repeat,
    ShowWindow,
    ToggleOverlay,
}
