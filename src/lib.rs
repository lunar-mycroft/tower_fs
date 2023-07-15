use std::{
    fs::Permissions,
    path::{Path, PathBuf},
    task::Poll,
};

use futures::future::{BoxFuture, FutureExt, TryFutureExt};
use tokio::fs;
use tower_service::Service;

#[derive(Debug, Clone, Copy)]
pub struct FileSystem;

impl<'a> Service<Request<'a>> for FileSystem {
    type Response = Response;
    type Error = std::io::Error;
    type Future = BoxFuture<'a, Result<Response, Self::Error>>;

    /// The `FileSystem` performs no setup of it's own, so it's ready immediately
    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<'a>) -> Self::Future {
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
            Request::Open { options, path } => {
                async move { options.open(path).await.map(Response::File) }.boxed()
            }
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

#[derive(Debug, Clone)]
pub enum Request<'a> {
    Copy {
        from: &'a Path,
        to: &'a Path,
    },
    CreateDir {
        path: &'a Path,
        recursive: bool,
    },
    FollowLink(&'a Path),
    GetMetadata {
        path: &'a Path,
        follow_symlinks: bool,
    },
    HardLink {
        src: &'a Path,
        dst: &'a Path,
    },
    Open {
        options: fs::OpenOptions,
        path: &'a Path,
    },
    RemoveDir {
        path: &'a Path,
        recursive: bool,
    },
    RemoveFile(&'a Path),
    Rename {
        from: &'a Path,
        to: &'a Path,
    },
    SetPermissions {
        path: &'a Path,
        perm: Permissions,
    },
    #[cfg(unix)]
    SymLink {
        src: &'a Path,
        dst: &'a Path,
    },
    #[cfg(windows)]
    SymlinkDir {
        src: &'a Path,
        dst: &'a Path,
    },
    SymlinkFile {
        src: &'a Path,
        dst: &'a Path,
    },
    Exists(&'a Path),
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
