mod errors;
mod term;
mod app;

use errors::Result;
use app::App;

fn main() -> Result<()> {
    App::new()?.run();
    Ok(())
}
