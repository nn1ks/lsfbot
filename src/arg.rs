use clap::Clap;
use std::path::PathBuf;

#[derive(Clap)]
pub struct Args {
    #[clap(long, short, value_name = "path", parse(from_os_str))]
    pub config: PathBuf,
}
