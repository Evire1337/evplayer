//! What a key, the tray, the media keys or the mini player can ask the player to do (the same
//! commands as runAction() in src/app/actions.ts).

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
    Search,
    Settings,
    NowPlaying,
    Quit,
    /// a key Windows would not register (another program has it)
    Noop,
}
