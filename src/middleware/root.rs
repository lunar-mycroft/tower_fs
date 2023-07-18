use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use futures::{
    future::{ready, BoxFuture},
    FutureExt,
};
use tower_layer::Layer;
use tower_service::Service;

use crate::{Request, Response};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RootLayer(PathBuf);

impl RootLayer {
    /// Converts the provided path to canonicalized absolute path and returns a [`RootLayer`] for that path
    ///
    /// # Errors
    ///
    /// Will fail if [`std::fs::canonicalize`][std] fails
    pub fn new<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        path.as_ref().canonicalize().map(Self)
    }
}

#[derive(Debug, Clone)]
pub struct Root<S> {
    root: PathBuf,
    inner: S,
}

impl<S> Service<Request> for Root<S>
where
    S: Service<Request, Error = std::io::Error, Response = Response>,
    S::Future: 'static + Send,
{
    type Response = Response;
    type Error = std::io::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        match req.adjust_paths(&self.root) {
            Some(req) => self.inner.call(req).boxed(),
            None => ready(Err(ErrorKind::NotFound.into())).boxed(),
        }
    }
}

impl<S: Service<Request>> Layer<S> for RootLayer {
    type Service = Root<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Root {
            root: self.0.clone(),
            inner,
        }
    }
}

fn make_relative(root: &Path, subpath: &Path) -> Option<PathBuf> {
    root.join(subpath.strip_prefix("/").unwrap_or(subpath))
        .canonicalize()
        .ok()
        .filter(|path| path.starts_with(root))
}

impl crate::Request {
    fn adjust_paths(self, root: &Path) -> Option<Self> {
        Some(match self {
            Self::Copy { from, to } => Self::Copy {
                from: make_relative(root, &from)?,
                to: make_relative(root, &to)?,
            },
            Self::CreateDir { path, recursive } => Self::CreateDir {
                path: make_relative(root, &path)?,
                recursive,
            },
            Self::Exists(path) => Self::Exists(make_relative(root, &path)?),
            Self::FollowLink(path) => Self::FollowLink(make_relative(root, &path)?),
            Self::GetMetadata {
                path,
                follow_symlinks,
            } => Self::GetMetadata {
                path: make_relative(root, &path)?,
                follow_symlinks,
            },
            Self::HardLink { src, dst } => Self::HardLink {
                src: make_relative(root, &src)?,
                dst: make_relative(root, &dst)?,
            },
            Self::Open { mode, path } => Self::Open {
                mode,
                path: make_relative(root, &path)?,
            },
            Self::RemoveDir { path, recursive } => Self::RemoveDir {
                path: make_relative(root, &path)?,
                recursive,
            },
            Self::RemoveFile(path) => Self::RemoveFile(make_relative(root, &path)?),
            Self::Rename { from, to } => Self::Rename {
                from: make_relative(root, &from)?,
                to: make_relative(root, &to)?,
            },
            Self::SetPermissions { path, perm } => Self::SetPermissions {
                path: make_relative(root, &path)?,
                perm,
            },
            #[cfg(windows)]
            Self::SymlinkDir { src, dst } => Self::SymlinkDir {
                src: make_relative(root, &src)?,
                dst: make_relative(root, &dst)?,
            },
            #[cfg(windows)]
            Self::SymlinkFile { src, dst } => Self::SymlinkFile {
                src: make_relative(root, &src)?,
                dst: make_relative(root, &dst)?,
            },
            #[cfg(unix)]
            Self::Symlink { src, dst } => Self::Symlink {
                src: make_relative(root, &src)?,
                dst: make_relative(root, &dst)?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relative() {
        assert_eq!(
            make_relative("".as_ref(), "/src".as_ref()),
            std::fs::canonicalize("src").ok()
        );
    }
}
