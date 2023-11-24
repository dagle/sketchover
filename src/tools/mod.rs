use raqote::{DrawTarget, Point};

use smithay_client_toolkit::seat::keyboard::Modifiers;

pub mod draw;

pub trait Tool {
    /// When the mouse is moved, the currently active (if there is one) tool
    /// will be updated,
    fn update(&mut self, motion: (f64, f64));
    /// convert the tool to a set of paths
    fn draw(&self, dt: &mut DrawTarget<&mut [u32]>);

    /// A tool sould be able to have a set of modifiers width
    /// it. Atm these are hardcoded against keys but in the future
    /// these should be mapped against a config value.
    fn modifier(&mut self, modifier: &Modifiers) {
        return;
    }
    /// For drawing the size of the figure
    /// The function returns the size (width, height) and a position
    /// to put the text
    fn draw_size(&self) -> Option<((f64, f64), Point)> {
        None
    }
}
