use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComputerToolCall {
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoordParam {
    /// The x-coordinate.
    #[serde(rename = "x")]
    pub x: i32,
    /// The y-coordinate.
    #[serde(rename = "y")]
    pub y: i32,
}

impl CoordParam {
    /// An x/y coordinate pair, e.g. `{ x: 100, y: 200 }`.
    pub fn new(x: i32, y: i32) -> CoordParam {
        CoordParam { x, y }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyPressAction {
    /// Specifies the event type. For a keypress action, this property is always set to `keypress`.
    #[serde(rename = "type")]
    pub r#type: KeyPressActionType,
    /// The combination of keys the model is requesting to be pressed. This is an array of strings, each representing a key.
    #[serde(rename = "keys")]
    pub keys: Vec<String>,
}

impl KeyPressAction {
    /// A collection of keypresses the model would like to perform.
    pub fn new(r#type: KeyPressActionType, keys: Vec<String>) -> KeyPressAction {
        KeyPressAction { r#type, keys }
    }
}
/// Specifies the event type. For a keypress action, this property is always set to `keypress`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum KeyPressActionType {
    #[serde(rename = "keypress")]
    #[default]
    Keypress,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoveParam {
    /// Specifies the event type. For a move action, this property is always set to `move`.
    #[serde(rename = "type")]
    pub r#type: MoveParamType,
    /// The x-coordinate to move to.
    #[serde(rename = "x")]
    pub x: i32,
    /// The y-coordinate to move to.
    #[serde(rename = "y")]
    pub y: i32,
    /// The keys being held while moving the mouse.
    #[serde(
        rename = "keys",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub keys: Option<Option<Vec<String>>>,
}

impl MoveParam {
    /// A mouse move action.
    pub fn new(r#type: MoveParamType, x: i32, y: i32) -> MoveParam {
        MoveParam { r#type, x, y, keys: None }
    }
}
/// Specifies the event type. For a move action, this property is always set to `move`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum MoveParamType {
    #[serde(rename = "move")]
    #[default]
    Move,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotParam {
    /// Specifies the event type. For a screenshot action, this property is always set to `screenshot`.
    #[serde(rename = "type")]
    pub r#type: ScreenshotParamType,
}

impl ScreenshotParam {
    /// A screenshot action.
    pub fn new(r#type: ScreenshotParamType) -> ScreenshotParam {
        ScreenshotParam { r#type }
    }
}
/// Specifies the event type. For a screenshot action, this property is always set to `screenshot`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ScreenshotParamType {
    #[serde(rename = "screenshot")]
    #[default]
    Screenshot,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScrollParam {
    /// Specifies the event type. For a scroll action, this property is always set to `scroll`.
    #[serde(rename = "type")]
    pub r#type: ScrollParamType,
    /// The x-coordinate where the scroll occurred.
    #[serde(rename = "x")]
    pub x: i32,
    /// The y-coordinate where the scroll occurred.
    #[serde(rename = "y")]
    pub y: i32,
    /// The horizontal scroll distance.
    #[serde(rename = "scroll_x")]
    pub scroll_x: i32,
    /// The vertical scroll distance.
    #[serde(rename = "scroll_y")]
    pub scroll_y: i32,
    /// The keys being held while scrolling.
    #[serde(
        rename = "keys",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub keys: Option<Option<Vec<String>>>,
}

impl ScrollParam {
    /// A scroll action.
    pub fn new(
        r#type: ScrollParamType,
        x: i32,
        y: i32,
        scroll_x: i32,
        scroll_y: i32,
    ) -> ScrollParam {
        ScrollParam { r#type, x, y, scroll_x, scroll_y, keys: None }
    }
}
/// Specifies the event type. For a scroll action, this property is always set to `scroll`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ScrollParamType {
    #[serde(rename = "scroll")]
    #[default]
    Scroll,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaitParam {
    /// Specifies the event type. For a wait action, this property is always set to `wait`.
    #[serde(rename = "type")]
    pub r#type: WaitParamType,
}

impl WaitParam {
    /// A wait action.
    pub fn new(r#type: WaitParamType) -> WaitParam {
        WaitParam { r#type }
    }
}
/// Specifies the event type. For a wait action, this property is always set to `wait`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum WaitParamType {
    #[serde(rename = "wait")]
    #[default]
    Wait,
}

