#[diagnostic::on_unimplemented(
       message = "`{Self}` must implement `Visualize` to derive `CopperVisualizer`",
       note = "add an `impl Visualize for {Self}` block"
   )]
pub trait Visualize {
   fn visualize(&mut self);
}
