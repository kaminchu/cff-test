use rquickjs::{Ctx, Result};
pub fn init<'js>(ctx: &Ctx<'js>) -> Result<()> {
    llrt_buffer::init(ctx)?;
    ctx.eval::<(), _>(include_str!("buffer.js"))
}
