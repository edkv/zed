use std::marker::PhantomData;
use std::rc::Rc;

use gpui3::{DefiniteLength, Hsla, MouseButton, WindowContext};

use crate::prelude::*;
use crate::{h_stack, theme, Icon, IconColor, IconElement, Label, LabelColor, LabelSize};

#[derive(Default, PartialEq, Clone, Copy)]
pub enum IconPosition {
    #[default]
    Left,
    Right,
}

#[derive(Default, Copy, Clone, PartialEq)]
pub enum ButtonVariant {
    #[default]
    Ghost,
    Filled,
}

// struct ButtonHandlers<V> {
//     click: Option<Rc<dyn Fn(&mut V, &mut EventContext<V>)>>,
// }

// impl<V> Default for ButtonHandlers<V> {
//     fn default() -> Self {
//         Self { click: None }
//     }
// }

#[derive(Element)]
pub struct Button<S: 'static + Send + Sync + Clone> {
    state_type: PhantomData<S>,
    label: String,
    variant: ButtonVariant,
    state: InteractionState,
    icon: Option<Icon>,
    icon_position: Option<IconPosition>,
    width: Option<DefiniteLength>,
    // handlers: ButtonHandlers<S>,
}

impl<S: 'static + Send + Sync + Clone> Button<S> {
    pub fn new<L>(label: L) -> Self
    where
        L: Into<String>,
    {
        Self {
            state_type: PhantomData,
            label: label.into(),
            variant: Default::default(),
            state: Default::default(),
            icon: None,
            icon_position: None,
            width: Default::default(),
            // handlers: ButtonHandlers::default(),
        }
    }

    pub fn ghost<L>(label: L) -> Self
    where
        L: Into<String>,
    {
        Self::new(label).variant(ButtonVariant::Ghost)
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn state(mut self, state: InteractionState) -> Self {
        self.state = state;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn icon_position(mut self, icon_position: IconPosition) -> Self {
        if self.icon.is_none() {
            panic!("An icon must be present if an icon_position is provided.");
        }
        self.icon_position = Some(icon_position);
        self
    }

    pub fn width(mut self, width: Option<DefiniteLength>) -> Self {
        self.width = width;
        self
    }

    // pub fn on_click(mut self, handler: impl Fn(&mut S, &mut EventContext<S>) + 'static) -> Self {
    //     self.handlers.click = Some(Rc::new(handler));
    //     self
    // }

    fn background_color(&self, cx: &mut ViewContext<S>) -> Hsla {
        let theme = theme(cx);
        let system_color = SystemColor::new();

        match (self.variant, self.state) {
            (ButtonVariant::Ghost, InteractionState::Hovered) => {
                theme.lowest.base.hovered.background
            }
            (ButtonVariant::Ghost, InteractionState::Active) => {
                theme.lowest.base.pressed.background
            }
            (ButtonVariant::Filled, InteractionState::Enabled) => {
                theme.lowest.on.default.background
            }
            (ButtonVariant::Filled, InteractionState::Hovered) => {
                theme.lowest.on.hovered.background
            }
            (ButtonVariant::Filled, InteractionState::Active) => theme.lowest.on.pressed.background,
            (ButtonVariant::Filled, InteractionState::Disabled) => {
                theme.lowest.on.disabled.background
            }
            _ => system_color.transparent,
        }
    }

    fn label_color(&self) -> LabelColor {
        match self.state {
            InteractionState::Disabled => LabelColor::Disabled,
            _ => Default::default(),
        }
    }

    fn icon_color(&self) -> IconColor {
        match self.state {
            InteractionState::Disabled => IconColor::Disabled,
            _ => Default::default(),
        }
    }

    fn border_color(&self, cx: &WindowContext) -> Hsla {
        let theme = theme(cx);
        let system_color = SystemColor::new();

        match self.state {
            InteractionState::Focused => theme.lowest.accent.default.border,
            _ => system_color.transparent,
        }
    }

    fn render_label(&self) -> Label<S> {
        Label::new(self.label.clone())
            .size(LabelSize::Small)
            .color(self.label_color())
    }

    fn render_icon(&self, icon_color: IconColor) -> Option<IconElement<S>> {
        self.icon.map(|i| IconElement::new(i).color(icon_color))
    }

    fn render(&mut self, cx: &mut ViewContext<S>) -> impl Element<State = S> {
        let theme = theme(cx);
        let icon_color = self.icon_color();
        let system_color = SystemColor::new();
        let border_color = self.border_color(cx);

        let mut el = h_stack()
            .h_6()
            .px_1()
            .items_center()
            .rounded_md()
            .border()
            .border_color(border_color)
            .fill(self.background_color(cx));

        match (self.icon, self.icon_position) {
            (Some(_), Some(IconPosition::Left)) => {
                el = el
                    .gap_1()
                    .child(self.render_label())
                    .children(self.render_icon(icon_color))
            }
            (Some(_), Some(IconPosition::Right)) => {
                el = el
                    .gap_1()
                    .children(self.render_icon(icon_color))
                    .child(self.render_label())
            }
            (_, _) => el = el.child(self.render_label()),
        }

        if let Some(width) = self.width {
            el = el.w(width).justify_center();
        }

        // if let Some(click_handler) = self.handlers.click.clone() {
        //     el = el.on_mouse_down(MouseButton::Left, move |view, event, cx| {
        //         click_handler(view, cx);
        //     });
        // }

        el
    }
}
