//! Individual read-only checks. Each one talks to the filesystem only
//! through [`crate::Root`].

mod buildkit;
mod deleted_open;
mod df_du_gap;
mod docker_logs;
mod inodes;
mod journald;
mod mountinfo;
mod util;

pub(crate) use buildkit::check as buildkit;
pub(crate) use deleted_open::check as deleted_open;
pub(crate) use df_du_gap::check as df_du_gap;
pub(crate) use docker_logs::check as docker_logs;
pub(crate) use inodes::check as inodes;
pub(crate) use journald::check as journald;
