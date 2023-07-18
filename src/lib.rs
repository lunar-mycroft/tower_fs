use std::{fs::Permissions, path::PathBuf, task::Poll};

use futures::future::{BoxFuture, FutureExt, TryFutureExt};
use tokio::fs;
use tower_service::Service;

#[cfg(feature = "http")]
pub mod http;
#[cfg(feature = "middleware")]
pub mod middleware;

#[derive(Debug, Clone, Copy)]
pub struct FileSystem;

impl Service<Request> for FileSystem {
    type Response = Response;
    type Error = std::io::Error;
    type Future = BoxFuture<'static, Result<Response, Self::Error>>;

    /// The [`FileSystem`] performs no setup of it's own, so it's ready immediately
    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        match req {
            Request::Copy { from, to } => fs::copy(from, to).map_ok(Response::Copied).boxed(),
            Request::CreateDir {
                path,
                recursive: true,
            } => fs::create_dir_all(path).map_ok(Response::done).boxed(),
            Request::CreateDir {
                path,
                recursive: false,
            } => fs::create_dir(path).map_ok(Response::done).boxed(),
            Request::Exists(path) => fs::try_exists(path).map_ok(Response::Exists).boxed(),
            Request::FollowLink(path) => fs::read_link(path).map_ok(Response::PointsTo).boxed(),
            Request::GetMetadata {
                path,
                follow_symlinks: true,
            } => fs::metadata(path).map_ok(Response::Metadata).boxed(),
            Request::GetMetadata {
                path,
                follow_symlinks: false,
            } => fs::symlink_metadata(path)
                .map_ok(Response::Metadata)
                .boxed(),
            Request::HardLink { src, dst } => {
                fs::hard_link(src, dst).map_ok(Response::done).boxed()
            }
            Request::Open { mode, path } => async move {
                mode.into_open_options()
                    .open(path)
                    .await
                    .map(Response::File)
            }
            .boxed(),
            Request::RemoveDir {
                path,
                recursive: true,
            } => fs::remove_dir_all(path).map_ok(Response::done).boxed(),
            Request::RemoveDir {
                path,
                recursive: false,
            } => fs::remove_dir(path).map_ok(Response::done).boxed(),
            Request::RemoveFile(path) => fs::remove_file(path).map_ok(Response::done).boxed(),
            Request::Rename { from, to } => fs::rename(from, to).map_ok(Response::done).boxed(),
            Request::SetPermissions { path, perm } => fs::set_permissions(path, perm)
                .map_ok(Response::done)
                .boxed(),
            #[cfg(unix)]
            Request::SymLink { src, dst } => fs::symlink(src, dst).map_ok(Response::done).boxed(),
            #[cfg(windows)]
            Request::SymlinkDir { src, dst } => {
                fs::symlink_dir(src, dst).map_ok(Response::done).boxed()
            }

            Request::SymlinkFile { src, dst } => {
                fs::symlink_file(src, dst).map_ok(Response::done).boxed()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Read,
    AppendExisting,
    CreateOrOverwrite,
    CreateOrAppend,
    CreateNew,
}

impl Mode {
    fn into_open_options(self) -> fs::OpenOptions {
        let mut options = fs::OpenOptions::new();
        match self {
            Self::Read => options.read(true),
            Self::AppendExisting => options.append(true),
            Self::CreateOrOverwrite => options.write(true).truncate(true),
            Self::CreateOrAppend => options.append(true).create(true),
            Self::CreateNew => options.write(true).create_new(true),
        };
        options
    }
}

#[derive(Debug, Clone)]
pub enum Request {
    Copy {
        from: PathBuf,
        to: PathBuf,
    },
    CreateDir {
        path: PathBuf,
        recursive: bool,
    },
    FollowLink(PathBuf),
    GetMetadata {
        path: PathBuf,
        follow_symlinks: bool,
    },
    HardLink {
        src: PathBuf,
        dst: PathBuf,
    },
    Open {
        mode: Mode,
        path: PathBuf,
    },
    RemoveDir {
        path: PathBuf,
        recursive: bool,
    },
    RemoveFile(PathBuf),
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    SetPermissions {
        path: PathBuf,
        perm: Permissions,
    },
    #[cfg(unix)]
    SymLink {
        src: PathBuf,
        dst: PathBuf,
    },
    #[cfg(windows)]
    SymlinkDir {
        src: PathBuf,
        dst: PathBuf,
    },
    SymlinkFile {
        src: PathBuf,
        dst: PathBuf,
    },
    Exists(PathBuf),
}

#[derive(Debug)]
pub enum Response {
    Done,
    Copied(u64),
    File(fs::File),
    Directory(Vec<(PathBuf, std::fs::Metadata)>),
    Metadata(std::fs::Metadata),
    Exists(bool),
    PointsTo(PathBuf),
}

impl Response {
    fn done(_: ()) -> Self {
        Self::Done
    }
}
