//! One way for a view to ask for something.
//!
//! Views are `RenderOnce` and have no entity of their own, so they cannot call
//! `cx.listener`. Rather than thread a different callback into each of them,
//! every view is handed the same dispatcher and emits an
//! [`Action`](crate::state::Action).
//!
//! The indirection earns its keep three times over: a click and a key binding
//! produce the same value, the command palette is a list of those values, and
//! interaction becomes testable without a window because an `Action` is plain
//! data.

use crate::state::Action;
use gpui_kit::prelude::*;
use gpui_kit::{App, ClickEvent, Div, Stateful, Window};
use std::rc::Rc;

/// What a view calls to ask for something.
pub type Dispatch = Rc<dyn Fn(Action, &mut Window, &mut App)>;

/// A dispatcher that does nothing.
///
/// For tests and for rendering a view outside an application. It is a real
/// dispatcher rather than an `Option`, so no view has a "not interactive"
/// branch to get wrong.
pub fn ignore() -> Dispatch {
    Rc::new(|_, _, _| {})
}

/// Make an element clickable, emitting `action`.
///
/// The pointer cursor is part of the contract: an element that responds to a
/// click and does not say so is a control the user has to discover by
/// accident.
pub fn clickable(element: Stateful<Div>, dispatch: &Dispatch, action: Action) -> Stateful<Div> {
    let dispatch = Rc::clone(dispatch);
    element.cursor_pointer().on_click(move |_: &ClickEvent, window, cx| {
        dispatch(action.clone(), window, cx);
    })
}
